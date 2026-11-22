use super::*;
use anyhow::{Result, anyhow};
use serde_json::Value;

struct FaultyDevices {
    fail_apply: Arc<AtomicBool>,
}

impl Devices for FaultyDevices {
    fn begin(&mut self) -> Result<()> {
        Ok(())
    }
    fn apply(&mut self, _: &pb::Targets) -> Result<()> {
        if self.fail_apply.load(Ordering::SeqCst) {
            return Err(anyhow!("模拟串口断开"));
        }
        Ok(())
    }
    fn configure_eyes(&mut self, _: &Value, _: bool) -> Result<()> {
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
    fn stop_chassis(&mut self) -> Result<()> {
        Ok(())
    }
    fn zero(&mut self, _: u8) -> Result<()> {
        Ok(())
    }
    fn chassis(&mut self, _: &pb::ChassisTarget) -> Result<()> {
        Ok(())
    }
    fn poll(&mut self) -> Result<()> {
        Ok(())
    }
    fn snapshot(&self) -> Value {
        serde_json::json!({})
    }
}

fn expect_err(result: Result<Reply, Status>) -> Status {
    match result {
        Ok(_) => panic!("预期返回错误"),
        Err(status) => status,
    }
}

fn new_test_engine() -> Engine {
    let mut e = Engine::new(
        Store::open(":memory:", None).unwrap(),
        true,
        tokio::runtime::Handle::current(),
    )
    .unwrap();
    e.reconnect();
    e
}

#[tokio::test]
async fn clear_latch_recovers_after_io_fault() {
    let mut e = new_test_engine();
    let fail = Arc::new(AtomicBool::new(true));
    e.devices = Some(Box::new(FaultyDevices {
        fail_apply: fail.clone(),
    }));

    let Reply::Lease(lease) = e
        .process(Op::Acquire(pb::AcquireRequest {
            owner: "tester".into(),
            ttl_ms: 1000,
            clear_latch: false,
        }))
        .unwrap()
    else {
        panic!("缺少租约");
    };
    assert_eq!(lease.token.len(), 32);

    let err = expect_err(e.process(Op::Targets(pb::TargetRequest {
        token: lease.token,
        targets: Some(pb::Targets {
            neck: Some(pb::NeckTarget {
                yaw_rad: Some(0.1),
                pitch_rad: None,
            }),
            ..Default::default()
        }),
    })));
    assert_eq!(err.code(), tonic::Code::Unavailable);
    assert!(e.latched);
    assert!(e.devices.is_none());
    assert!(!e.fault.is_empty());
    assert_eq!(e.applied, 0);

    let denied = expect_err(e.process(Op::Acquire(pb::AcquireRequest {
        owner: "tester".into(),
        ttl_ms: 1000,
        clear_latch: false,
    })));
    assert_eq!(denied.code(), tonic::Code::FailedPrecondition);

    let Reply::Lease(recovered) = e
        .process(Op::Acquire(pb::AcquireRequest {
            owner: "tester".into(),
            ttl_ms: 1000,
            clear_latch: true,
        }))
        .unwrap()
    else {
        panic!("clear_latch 应重建设备并恢复租约");
    };
    assert_eq!(recovered.token.len(), 32);
    assert!(!e.latched);
    assert!(e.fault.is_empty());
    assert_eq!(e.applied, e.config_revision);
    assert!(e.devices.is_some());
}

#[tokio::test]
async fn config_update_is_not_blocked_by_existing_actions() {
    let mut e = new_test_engine();
    let mut cfg = e.cfg.clone();
    cfg.left_arm.enabled = true;
    e.process(Op::Update(pb::ConfigUpdate {
        expected_revision: e.config_revision,
        config: Some(crate::to_struct(&cfg.value())),
    }))
    .unwrap();

    let action = pb::Action {
        name: "抬臂".into(),
        duration_ms: 300,
        steps: vec![pb::ActionStep {
            event: None,
            at_ms: 0,
            targets: Some(pb::Targets {
                left_arm: Some(pb::ArmTarget {
                    joint_rad: vec![0.4, 0.0, 0.0, 0.0],
                    duration_ms: 200,
                }),
                ..Default::default()
            }),
        }],
        ..Default::default()
    };
    e.process(Op::Put(pb::ActionUpdate {
        expected_revision: 0,
        action: Some(action),
    }))
    .unwrap();

    // 收窄关节限位后并禁用左臂，历史动作库不应阻止配置更新。
    cfg.left_arm.joints[0].min_rad = -0.2;
    cfg.left_arm.joints[0].max_rad = 0.2;
    cfg.left_arm.enabled = false;
    e.process(Op::Update(pb::ConfigUpdate {
        expected_revision: e.config_revision,
        config: Some(crate::to_struct(&cfg.value())),
    }))
    .unwrap();

    let Reply::Lease(lease) = e
        .process(Op::Acquire(pb::AcquireRequest {
            owner: "tester".into(),
            ttl_ms: 1000,
            clear_latch: false,
        }))
        .unwrap()
    else {
        panic!("缺少租约");
    };
    // 真正运行动作时仍应拦截与当前配置不兼容的动作。
    let run_err = expect_err(e.process(Op::Run(pb::RunRequest {
        token: lease.token,
        name: "抬臂".into(),
    })));
    assert_eq!(run_err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn action_step_delay_does_not_latch_or_drop_devices() {
    let mut e = new_test_engine();
    e.action = Some(ActiveAction {
        plan: pb::Action {
            name: "点头".into(),
            duration_ms: 1000,
            steps: vec![pb::ActionStep {
                event: None,
                at_ms: 0,
                targets: Some(pb::Targets {
                    neck: Some(pb::NeckTarget {
                        yaw_rad: Some(0.1),
                        pitch_rad: None,
                    }),
                    ..Default::default()
                }),
            }],
            ..Default::default()
        },
        start: Instant::now() - Duration::from_millis(500),
        next: 0,
        relative: false,
        curve: pb::MotionCurve::ServoNative,
        previous: Default::default(),
        segment: None,
    });

    e.tick();
    assert!(e.fault.is_empty());
    assert!(!e.latched);
    assert!(e.devices.is_some());
    assert!(!e.stopped);
    assert_eq!(e.action.as_ref().unwrap().next, 1);
}
#[test]
fn arm_curve_samples_between_known_poses_and_lands_after_a_late_tick() {
    let pose = |value| pb::Targets {
        left_arm: Some(pb::ArmTarget {
            joint_rad: vec![value; 4],
            duration_ms: 400,
        }),
        ..Default::default()
    };
    for (curve, quarter) in [
        (pb::MotionCurve::SCurve, 0.15625),
        (pb::MotionCurve::SCurveStrong, 0.070556640625),
    ] {
        let mut previous = pb::Targets::default();
        assert!(ArmSegment::begin(&mut previous, &pose(0.0), 0).is_none());
        let mut segment = ArmSegment::begin(&mut previous, &pose(1.0), 400).unwrap();
        let mut sample = pb::Targets::default();
        segment.sample(60, curve, &mut sample);
        assert_eq!(
            sample.left_arm.as_ref().unwrap().joint_rad,
            vec![quarter; 4]
        );
        segment.sample(420, curve, &mut sample);
        assert_eq!(
            sample.left_arm.unwrap(),
            pb::ArmTarget {
                joint_rad: vec![1.0; 4],
                duration_ms: 1
            }
        );
        assert!(sample.right_arm.is_none());
    }
}
