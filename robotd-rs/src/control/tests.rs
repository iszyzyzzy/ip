use super::*;
use anyhow::Result;
use serde_json::Value;

struct TestArm {
    pending: Arc<AtomicBool>,
    events: Sender<&'static str>,
}

impl Devices for TestArm {
    fn waiting_for_reply(&self, t: &pb::Targets) -> bool {
        t.servo.is_some() && self.pending.load(Ordering::SeqCst)
    }
    fn poll(&mut self, start_arm_query: bool) -> Result<()> {
        if !start_arm_query {
            self.events.send("等待回包")?;
        }
        Ok(())
    }
    fn apply(&mut self, t: &pb::Targets) -> Result<()> {
        assert!(!self.waiting_for_reply(t), "回包期间写入了同一总线");
        self.events.send("运动")?;
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.pending.store(false, Ordering::SeqCst);
        let _ = self.events.send("停止");
        Ok(())
    }
    fn snapshot(&self) -> Value {
        serde_json::json!({})
    }
    fn begin(&mut self) -> Result<()> {
        Ok(())
    }
    fn configure_eyes(&mut self, _: &Value, _: bool) -> Result<()> {
        unreachable!()
    }
    fn stop_chassis(&mut self) -> Result<()> {
        unreachable!()
    }
    fn zero(&mut self, _: u8) -> Result<()> {
        unreachable!()
    }
    fn chassis(&mut self, _: &pb::ChassisTarget) -> Result<()> {
        unreachable!()
    }
}

#[tokio::test]
async fn position_query_gates_motion_and_allows_stop() {
    let mut e = Engine::new(
        Store::open(":memory:", None).unwrap(),
        true,
        tokio::runtime::Handle::current(),
    )
    .unwrap();
    e.applied = e.config_revision;
    e.cfg.left_arm.enabled = true;
    let pending = Arc::new(AtomicBool::new(true));
    let (events, trace) = crossbeam_channel::unbounded();
    e.devices = Some(Box::new(TestArm {
        pending: pending.clone(),
        events,
    }));
    let (normal, nrx) = crossbeam_channel::unbounded();
    let (urgent, urx) = crossbeam_channel::unbounded();
    let worker = std::thread::spawn(move || e.run(nrx, urx, Arc::new(AtomicBool::new(false))));
    let submit = |tx: &Sender<Job>, op| {
        let (reply, rx) = oneshot::channel();
        tx.send(Job {
            op,
            reply,
            deadline: Instant::now() + Duration::from_secs(3),
        })
        .unwrap();
        rx
    };
    let Reply::Lease(lease) = submit(
        &normal,
        Op::Acquire(pb::AcquireRequest {
            owner: "test".into(),
            ttl_ms: 5000,
            ..Default::default()
        }),
    )
    .await
    .unwrap()
    .unwrap() else {
        panic!("缺少租约")
    };
    let movement = || {
        Op::Targets(pb::TargetRequest {
            token: lease.token.clone(),
            targets: Some(pb::Targets {
                servo: Some(pb::ServoTarget {
                    position_pwm: 1500,
                    ..Default::default()
                }),
                ..Default::default()
            }),
        })
    };
    let next = || trace.recv_timeout(Duration::from_secs(1)).unwrap();
    let first = submit(&normal, movement());
    assert_eq!(next(), "等待回包");
    pending.store(false, Ordering::SeqCst);
    first.await.unwrap().unwrap();
    while next() == "等待回包" {}
    pending.store(true, Ordering::SeqCst);
    let second = submit(&normal, movement());
    assert_eq!(next(), "等待回包");
    submit(&urgent, Op::Stop(pb::StopRequest { latch: true }))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(second.await.unwrap(), Err(s) if s.code() == tonic::Code::Cancelled));
    drop(normal);
    worker.join().unwrap();
    assert!(!trace.try_iter().any(|event| event == "运动"));
}
