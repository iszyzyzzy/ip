use crate::{
    audio::{Player, Session},
    config::Config,
    drivers::{Devices, Hardware, Simulated},
    pb,
    store::Store,
    validation,
};
use crossbeam_channel::{Receiver, Sender};
use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use tonic::Status;

pub enum Op {
    Config,
    Update(pb::ConfigUpdate),
    Eyes(pb::EyesUpdate),
    Acquire(pb::AcquireRequest),
    Renew(pb::LeaseRequest),
    Release(pb::LeaseRequest),
    Targets(pb::TargetRequest),
    Stop(pb::StopRequest),
    Zero(pb::ZeroRequest),
    Actions,
    Put(pb::ActionUpdate),
    Capture(pb::PresetCapture),
    Delete(pb::ActionDelete),
    Run(pb::RunRequest),
    Cancel(pb::LeaseRequest),
    Audio(pb::AudioHeader),
    Shutdown,
}
#[allow(clippy::large_enum_variant)]
pub enum Reply {
    Config(pb::ConfigSnapshot),
    Lease(pb::Lease),
    Ack(pb::Ack),
    Actions(pb::ActionList),
    Action(pb::ActionRecord),
    Audio(Arc<Session>),
}
struct Job {
    op: Op,
    reply: oneshot::Sender<Result<Reply, Status>>,
    deadline: Instant,
}
#[derive(Clone)]
pub struct Hub {
    normal: Sender<Job>,
    urgent: Sender<Job>,
    pub state: Arc<RwLock<pb::RobotState>>,
    pub audio: Player,
    stopping: Arc<AtomicBool>,
    worker: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}
impl Hub {
    pub fn start(
        store: Store,
        simulated: bool,
        rt: tokio::runtime::Handle,
    ) -> anyhow::Result<Self> {
        let mut engine = Engine::new(store, simulated, rt)?;
        let audio = engine.audio.clone();
        let state = engine.state.clone();
        let (normal, nrx) = crossbeam_channel::bounded(64);
        let (urgent, urx) = crossbeam_channel::bounded(8);
        let stopping = Arc::new(AtomicBool::new(false));
        let (ready, rx) = std::sync::mpsc::sync_channel(1);
        let stop = stopping.clone();
        let worker = std::thread::Builder::new()
            .name("robot-control".into())
            .stack_size(512 * 1024)
            .spawn(move || {
                engine.reconnect();
                engine.publish();
                let _ = ready.send(());
                engine.run(nrx, urx, stop);
            })?;
        rx.recv_timeout(Duration::from_secs(10))?;
        Ok(Self {
            normal,
            urgent,
            state,
            audio,
            stopping,
            worker: Arc::new(Mutex::new(Some(worker))),
        })
    }
    pub async fn request(&self, op: Op, urgent: bool) -> Result<Reply, Status> {
        if self.stopping.load(Ordering::Relaxed) {
            return Err(Status::unavailable("robotd 已停止"));
        }
        let (reply, rx) = oneshot::channel();
        let job = Job {
            op,
            reply,
            deadline: Instant::now() + Duration::from_secs(5),
        };
        (if urgent { &self.urgent } else { &self.normal })
            .try_send(job)
            .map_err(|_| Status::resource_exhausted("控制请求队列已满"))?;
        tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Status::deadline_exceeded("请求超时，请查询状态，不要自动重发"))?
            .map_err(|_| Status::unavailable("控制线程已退出"))?
    }
    pub fn snapshot(&self) -> pb::RobotState {
        let mut s = self.state.read().unwrap().clone();
        s.audio = Some(self.audio.status());
        s
    }
    pub async fn shutdown(&self) {
        let _ = self.request(Op::Shutdown, true).await;
        self.stopping.store(true, Ordering::Relaxed);
        self.audio.stop();
        let worker = self.worker.lock().unwrap().take();
        if let Some(w) = worker {
            let _ = tokio::task::spawn_blocking(move || w.join()).await;
        }
    }
}

struct ActiveAction {
    plan: pb::Action,
    start: Instant,
    next: usize,
    relative: bool,
    curve: pb::MotionCurve,
    previous: pb::Targets,
    segment: Option<ArmSegment>,
}

struct ArmSegment {
    at_ms: u32,
    duration: u32,
    next_ms: u32,
    arms: [Option<(pb::ArmTarget, pb::ArmTarget)>; 2],
}

impl ArmSegment {
    fn begin(previous: &mut pb::Targets, targets: &pb::Targets, at_ms: u32) -> Option<Self> {
        let mut duration = 0;
        let mut track = |last: &mut Option<pb::ArmTarget>, goal: &Option<pb::ArmTarget>| {
            let goal = goal.as_ref()?;
            let start = last.replace(goal.clone())?;
            if start.joint_rad == goal.joint_rad || goal.duration_ms <= 40 {
                return None;
            }
            duration = goal.duration_ms;
            Some((start, goal.clone()))
        };
        let arms = [
            track(&mut previous.left_arm, &targets.left_arm),
            track(&mut previous.right_arm, &targets.right_arm),
        ];
        (duration > 0).then_some(Self {
            at_ms,
            duration,
            next_ms: 0,
            arms,
        })
    }

    fn sample(&mut self, elapsed: u32, curve: pb::MotionCurve, targets: &mut pb::Targets) {
        let end = elapsed.saturating_add(40).min(self.duration);
        let t = f64::from(end) / f64::from(self.duration);
        let weight = match curve {
            pb::MotionCurve::SCurveStrong => {
                t.powi(4) * (35.0 + t * (-84.0 + t * (70.0 - 20.0 * t)))
            }
            _ => t * t * (3.0 - 2.0 * t),
        };
        for (pair, target) in self
            .arms
            .iter()
            .zip([&mut targets.left_arm, &mut targets.right_arm])
        {
            if let Some((start, goal)) = pair {
                let mut sample = goal.clone();
                if end < self.duration {
                    for (q, from) in sample.joint_rad.iter_mut().zip(&start.joint_rad) {
                        *q = from + (*q - from) * weight;
                    }
                }
                sample.duration_ms = end.saturating_sub(elapsed).max(1);
                *target = Some(sample);
            }
        }
        self.next_ms = end;
    }
}

struct Engine {
    store: Store,
    cfg: Config,
    config_revision: u64,
    applied: u64,
    revision: u64,
    simulated: bool,
    rt: tokio::runtime::Handle,
    devices: Option<Box<dyn Devices>>,
    state: Arc<RwLock<pb::RobotState>>,
    audio: Player,
    lease: Option<pb::Lease>,
    lease_end: Instant,
    action: Option<ActiveAction>,
    action_sound: Option<Arc<Session>>,
    chassis: Option<pb::ChassisTarget>,
    chassis_end: Instant,
    next_chassis: Instant,
    next_poll: Instant,
    stopped: bool,
    latched: bool,
    fault: String,
}
fn invalid(e: impl std::fmt::Display) -> Status {
    Status::invalid_argument(e.to_string())
}
#[cfg(test)]
mod tests;

impl Engine {
    fn new(store: Store, simulated: bool, rt: tokio::runtime::Handle) -> anyhow::Result<Self> {
        let (rev, cfg) = store.config()?;
        let audio = Player::new(cfg.audio.clone(), simulated);
        Ok(Self {
            store,
            cfg,
            config_revision: rev,
            applied: 0,
            revision: 0,
            simulated,
            rt,
            devices: None,
            state: Arc::new(RwLock::new(pb::RobotState::default())),
            audio,
            lease: None,
            lease_end: Instant::now(),
            action: None,
            action_sound: None,
            chassis: None,
            chassis_end: Instant::now(),
            next_chassis: Instant::now(),
            next_poll: Instant::now(),
            stopped: true,
            latched: false,
            fault: String::new(),
        })
    }

    fn reconnect(&mut self) {
        self.applied = 0;
        self.devices = None;
        self.audio.configure(self.cfg.audio.clone());
        let result: anyhow::Result<Box<dyn Devices>> = if self.simulated {
            Ok(Box::<Simulated>::default())
        } else {
            Hardware::connect(self.cfg.clone(), self.rt.clone())
                .map(|x| Box::new(x) as Box<dyn Devices>)
        };
        match result {
            Ok(d) => {
                self.devices = Some(d);
                self.applied = self.config_revision;
                self.fault.clear();
            }
            Err(e) => {
                self.fault = format!("设备连接/配置失败：{e}");
                self.latched = true;
                eprintln!("{}", self.fault);
            }
        }
    }
    fn run(&mut self, normal: Receiver<Job>, urgent: Receiver<Job>, stop: Arc<AtomicBool>) {
        while !stop.load(Ordering::Relaxed) {
            crossbeam_channel::select_biased! {
                recv(urgent) -> job => {
                    let Ok(job) = job else { break };
                    self.job(job);
                    continue;
                },
                recv(normal) -> job => {
                    let Ok(job) = job else { break };
                    self.job(job);
                },
                default(Duration::from_millis(5)) => {},
            }
            self.tick();
            self.publish();
        }
        self.stop_motion(true);
        self.audio.stop();
        self.devices = None;
        self.publish();
    }
    fn job(&mut self, job: Job) {
        if job.reply.is_closed() || Instant::now() >= job.deadline {
            return;
        }
        let shutdown = matches!(job.op, Op::Shutdown);
        let result = self.process(job.op);
        self.publish();
        let _ = job.reply.send(result);
        if shutdown {
            self.stop_motion(true);
        }
    }
    fn publish(&self) {
        let s = pb::RobotState {
            revision: self.revision,
            config_revision: self.config_revision,
            applied_config_revision: self.applied,
            simulated: self.simulated,
            stopped: self.stopped,
            latched: self.latched,
            owner: self
                .lease
                .as_ref()
                .map(|l| l.owner.clone())
                .unwrap_or_default(),
            lease_remaining_ms: if self.lease.is_some() {
                self.lease_end
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u32
            } else {
                0
            },
            action: self
                .action
                .as_ref()
                .map(|x| x.plan.name.clone())
                .unwrap_or_default(),
            fault: self.fault.clone(),
            devices: Some(crate::to_struct(
                &self
                    .devices
                    .as_ref()
                    .map(|d| d.snapshot())
                    .unwrap_or_else(|| serde_json::json!({})),
            )),
            audio: None,
        };
        *self.state.write().unwrap() = s;
    }
    fn lease(&self, token: &str) -> Result<(), Status> {
        if self.lease.as_ref().is_none_or(|l| l.token != token) || Instant::now() >= self.lease_end
        {
            return Err(Status::permission_denied(
                "控制权不存在、已到期或 token 无效",
            ));
        }
        if self.latched || !self.fault.is_empty() || self.applied != self.config_revision {
            return Err(Status::failed_precondition(
                "设备故障、停止锁定或配置尚未应用",
            ));
        }
        Ok(())
    }
    fn stop_motion(&mut self, revoke: bool) {
        if let Some(sound) = self.action_sound.take() {
            sound.cancel();
        }
        self.finish_motion(revoke);
    }
    fn finish_motion(&mut self, revoke: bool) {
        self.action = None;
        self.chassis = None;
        self.stopped = true;
        self.revision += 1;
        if revoke {
            self.lease = None;
        }
        if let Some(d) = &mut self.devices
            && let Err(e) = d.stop()
        {
            self.fault = format!("设备停止失败：{e}");
            self.latched = true;
            self.applied = 0;
        }
    }
    fn io_result(&mut self, r: anyhow::Result<()>) -> Result<(), Status> {
        if let Err(e) = r {
            self.stop_motion(true);
            self.devices = None;
            self.fault = format!("设备控制失败：{e}");
            self.latched = true;
            self.applied = 0;
            eprintln!("{}", self.fault);
            return Err(Status::unavailable(self.fault.clone()));
        }
        Ok(())
    }
    fn apply(&mut self, t: &pb::Targets) -> Result<(), Status> {
        let result = self
            .devices
            .as_mut()
            .ok_or_else(|| Status::unavailable("设备未连接"))?
            .apply(t);
        self.io_result(result)?;
        if let Some(c) = &t.chassis {
            self.chassis = Some(*c);
            self.chassis_end = Instant::now() + Duration::from_millis(c.ttl_ms as u64);
            self.next_chassis = Instant::now();
        }
        self.stopped = false;
        self.revision += 1;
        Ok(())
    }
    fn ack(&self) -> Reply {
        Reply::Ack(pb::Ack {
            revision: self.revision,
        })
    }
    fn config(&self) -> Reply {
        Reply::Config(pb::ConfigSnapshot {
            revision: self.config_revision,
            config: Some(crate::to_struct(&self.cfg.value())),
        })
    }
    fn compile_action(&self, a: &pb::Action, cfg: &Config) -> Result<pb::Action, Status> {
        crate::actions::compile(a, cfg, |name| Ok(self.store.action(name)?)).map_err(invalid)
    }

    fn put_action(&mut self, action: pb::Action, expected: u64) -> Result<Reply, Status> {
        self.compile_action(&action, &self.cfg)?;
        if self
            .action
            .as_ref()
            .is_some_and(|x| x.plan.name == action.name)
        {
            return Err(Status::failed_precondition("动作执行中，不能修改"));
        }
        if !matches!(action.definition, Some(pb::action::Definition::Preset(_))) {
            self.ensure_unreferenced(&action.name)?;
        }
        let revision = self.store.put(&action, expected)?;
        Ok(Reply::Action(pb::ActionRecord {
            revision,
            action: Some(action),
            deleted: false,
        }))
    }

    fn ensure_unreferenced(&self, name: &str) -> Result<(), Status> {
        for record in self.store.actions()?.actions {
            let Some(action) = record.action else {
                continue;
            };
            if let Some(pb::action::Definition::Sequence(sequence)) = action.definition
                && sequence.steps.iter().any(|step| step.preset == name)
            {
                return Err(Status::failed_precondition(format!(
                    "预设仍被序列 {} 使用",
                    action.name
                )));
            }
        }
        Ok(())
    }

    fn process(&mut self, op: Op) -> Result<Reply, Status> {
        match op {
            Op::Config => Ok(self.config()),
            Op::Update(r) => {
                let cfg = Config::parse(crate::from_struct(r.config)?).map_err(invalid)?;
                if r.expected_revision != self.config_revision {
                    return Err(Status::aborted("配置版本不一致"));
                }
                self.stop_motion(true);
                self.audio.stop();
                self.config_revision = self.store.update(r.expected_revision, &cfg)?;
                self.cfg = cfg;
                self.reconnect();
                self.revision += 1;
                Ok(self.config())
            }
            Op::Eyes(r) => {
                if r.expected_revision != self.config_revision {
                    return Err(Status::aborted("配置版本不一致"));
                }
                if !self.cfg.eyes.enabled {
                    return Err(Status::failed_precondition("眼屏已禁用"));
                }
                let mut value = self.cfg.value();
                value["eyes"]["settings"] = crate::from_struct(r.settings)?;
                let cfg = Config::parse(value).map_err(invalid)?;
                let settings = &cfg.eyes.settings;
                // 眼屏预览失败只报告该操作，不停止正在运行的底盘和关节。
                self.devices
                    .as_mut()
                    .ok_or_else(|| Status::unavailable("设备未连接"))?
                    .configure_eyes(settings, r.connect_tracking)
                    .map_err(|e| Status::unavailable(format!("眼屏调节失败：{e}")))?;
                if r.save {
                    self.config_revision = self.store.update(r.expected_revision, &cfg)?;
                    self.cfg = cfg;
                    self.applied = self.config_revision;
                }
                self.revision += 1;
                Ok(self.config())
            }
            Op::Acquire(r) => {
                if self.lease.is_some() {
                    return Err(Status::resource_exhausted("控制权已占用"));
                }
                if !(200..=5000).contains(&r.ttl_ms)
                    || r.owner.trim().is_empty()
                    || r.owner.chars().count() > 128
                {
                    return Err(invalid("控制者名称或租约期限无效"));
                }
                if self.latched && !r.clear_latch {
                    return Err(Status::failed_precondition("停止已锁定，需要 clear_latch"));
                }
                if r.clear_latch
                    && (self.devices.is_none()
                        || !self.fault.is_empty()
                        || self.applied != self.config_revision)
                {
                    self.reconnect();
                }
                if !self.fault.is_empty()
                    || self.applied != self.config_revision
                    || self.devices.is_none()
                {
                    let msg = if self.fault.is_empty() {
                        "设备未连接或配置尚未应用".to_string()
                    } else {
                        self.fault.clone()
                    };
                    return Err(Status::failed_precondition(msg));
                }
                let result = self.devices.as_mut().unwrap().begin();
                self.io_result(result)?;
                self.latched = false;
                let l = pb::Lease {
                    token: uuid::Uuid::new_v4().simple().to_string(),
                    owner: r.owner,
                    ttl_ms: r.ttl_ms,
                };
                self.lease_end = Instant::now() + Duration::from_millis(r.ttl_ms as u64);
                self.lease = Some(l.clone());
                self.revision += 1;
                Ok(Reply::Lease(l))
            }
            Op::Renew(r) => {
                self.lease(&r.token)?;
                let l = self.lease.as_ref().unwrap().clone();
                self.lease_end = Instant::now() + Duration::from_millis(l.ttl_ms as u64);
                Ok(Reply::Lease(l))
            }
            Op::Release(r) => {
                self.lease(&r.token)?;
                self.stop_motion(true);
                Ok(self.ack())
            }
            Op::Targets(r) => {
                self.lease(&r.token)?;
                if self.action.is_some() {
                    return Err(Status::failed_precondition("动作执行中，先取消动作"));
                }
                let t = r.targets.ok_or_else(|| invalid("缺少 targets"))?;
                validation::targets(&t, &self.cfg).map_err(invalid)?;
                self.apply(&t)?;
                Ok(self.ack())
            }
            Op::Stop(r) => {
                self.latched |= r.latch;
                self.audio.stop();
                self.stop_motion(true);
                Ok(self.ack())
            }
            Op::Zero(r) => {
                self.lease(&r.token)?;
                if !self.cfg.neck.enabled || !(1..=3).contains(&r.mask) {
                    return Err(invalid("颈部未启用或 mask 无效"));
                }
                self.stop_motion(false);
                let result = self.devices.as_mut().unwrap().zero(r.mask as u8);
                self.io_result(result)?;
                Ok(self.ack())
            }
            Op::Actions => Ok(Reply::Actions(self.store.actions()?)),
            Op::Put(r) => self.put_action(
                r.action.ok_or_else(|| invalid("缺少 action"))?,
                r.expected_revision,
            ),
            Op::Capture(r) => {
                let state = self
                    .devices
                    .as_ref()
                    .ok_or_else(|| Status::unavailable("设备未连接"))?
                    .snapshot();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(invalid)?
                    .as_secs_f64();
                let action = crate::actions::capture(&r, &state, now)
                    .map_err(|e| Status::failed_precondition(e.to_string()))?;
                self.put_action(action, r.expected_revision)
            }
            Op::Delete(r) => {
                if self.action.as_ref().is_some_and(|x| x.plan.name == r.name) {
                    return Err(Status::failed_precondition("动作执行中，不能删除"));
                }
                self.ensure_unreferenced(&r.name)?;
                let rev = self.store.delete(&r.name, r.expected_revision)?;
                Ok(Reply::Ack(pb::Ack { revision: rev }))
            }
            Op::Run(r) => {
                self.lease(&r.token)?;
                if self.action.is_some() {
                    return Err(Status::failed_precondition("已有动作执行中"));
                }
                let action = self.store.action(&r.name)?;
                let plan = self.compile_action(&action, &self.cfg)?;
                self.stop_motion(false);
                if !self.fault.is_empty() {
                    return Err(Status::unavailable(self.fault.clone()));
                }
                self.action = Some(ActiveAction {
                    plan,
                    start: Instant::now(),
                    next: 0,
                    relative: action.definition.is_some(),
                    curve: match &action.definition {
                        Some(pb::action::Definition::Sequence(s)) => s.motion_curve(),
                        _ => pb::MotionCurve::ServoNative,
                    },
                    previous: Default::default(),
                    segment: None,
                });
                Ok(self.ack())
            }
            Op::Cancel(r) => {
                self.lease(&r.token)?;
                self.stop_motion(false);
                Ok(self.ack())
            }
            Op::Audio(h) => {
                if self.latched {
                    return Err(Status::failed_precondition("停止锁定，不能播放音频"));
                }
                Ok(Reply::Audio(self.audio.start(h)?))
            }
            Op::Shutdown => {
                self.stop_motion(true);
                self.audio.stop();
                Ok(self.ack())
            }
        }
    }
    fn tick(&mut self) {
        let now = Instant::now();
        if self.lease.is_some() && now >= self.lease_end {
            self.stop_motion(true);
        }
        if !self.fault.is_empty() {
            return;
        }
        if let Some(active) = &mut self.action {
            let elapsed = active.start.elapsed().as_millis();
            let finished = elapsed >= active.plan.duration_ms as u128;
            if let Some(segment) = &mut active.segment {
                let local = (elapsed as u32).saturating_sub(segment.at_ms);
                if local >= segment.next_ms {
                    let mut targets = pb::Targets::default();
                    segment.sample(local, active.curve, &mut targets);
                    let end = segment.at_ms + segment.duration;
                    let final_sample = segment.next_ms == segment.duration;
                    if final_sample {
                        active.segment = None;
                    }
                    if self.apply(&targets).is_err() {
                        return;
                    }
                    if final_sample {
                        let remaining = (end as u128).saturating_sub(elapsed).max(1);
                        self.action.as_mut().unwrap().start =
                            Instant::now() - Duration::from_millis(end as u64 - remaining as u64);
                    }
                }
            } else if let Some(step) = active
                .plan
                .steps
                .get(active.next)
                .filter(|s| elapsed >= s.at_ms as u128 && (active.relative || !finished))
                .cloned()
            {
                active.next += 1;
                let relative = active.relative;
                let result = match step.event.and_then(|event| event.kind) {
                    Some(pb::sequence_event::Kind::Sound(name)) => {
                        match self.audio.play_sound(&name, &self.rt) {
                            Ok(s) => self.action_sound = Some(s),
                            Err(e) => eprintln!("跳过动作语音 {name}：{e}"),
                        }
                        Ok(())
                    }
                    Some(pb::sequence_event::Kind::EyeExpression(e)) => self.apply(&pb::Targets {
                        eye_expression: Some(e),
                        ..Default::default()
                    }),
                    None => {
                        let mut targets = step.targets.unwrap();
                        if active.curve != pb::MotionCurve::ServoNative {
                            active.segment =
                                ArmSegment::begin(&mut active.previous, &targets, step.at_ms);
                            if let Some(segment) = &mut active.segment {
                                segment.sample(0, active.curve, &mut targets);
                            }
                        }
                        self.apply(&targets)
                    }
                };
                if result.is_err() {
                    return;
                }
                if relative {
                    // 按实际下发完成时间留足运动与停留，避免多设备串行发送吞掉时长。
                    self.action.as_mut().unwrap().start =
                        Instant::now() - Duration::from_millis(step.at_ms as u64);
                }
            } else if finished {
                // 自然结束时允许尾音播完，随后取消或释放控制权仍可停止它。
                self.finish_motion(false);
            }
        }
        if let Some(c) = self.chassis {
            if now >= self.chassis_end {
                self.chassis = None;
                let r = self.devices.as_mut().unwrap().stop_chassis();
                if self.io_result(r).is_err() {
                    return;
                }
            } else if now >= self.next_chassis {
                let r = self.devices.as_mut().unwrap().chassis(&c);
                if self.io_result(r).is_err() {
                    return;
                }
                self.next_chassis = now + Duration::from_millis(20);
            }
        }
        if now >= self.next_poll {
            if let Some(d) = &mut self.devices {
                let r = d.poll();
                let _ = self.io_result(r);
            }
            self.next_poll = now + Duration::from_millis(20);
        }
    }
}
