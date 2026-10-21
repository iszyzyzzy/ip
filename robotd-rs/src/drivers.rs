use crate::{
    config::{Arm, Config},
    eyes::v1 as ep,
    h7::Bridge,
    pb,
};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use serialport::SerialPort;
use std::{
    collections::HashMap,
    io::{Read, Write},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tonic::transport::{Channel, Endpoint};

pub trait Devices: Send {
    fn begin(&mut self) -> Result<()>;
    fn apply(&mut self, t: &pb::Targets) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn stop_chassis(&mut self) -> Result<()>;
    fn zero(&mut self, mask: u8) -> Result<()>;
    fn chassis(&mut self, t: &pb::ChassisTarget) -> Result<()>;
    fn poll(&mut self) -> Result<()>;
    fn snapshot(&self) -> Value;
}

#[derive(Default)]
pub struct Simulated {
    events: std::collections::VecDeque<Value>,
}
impl Simulated {
    fn record(&mut self, name: &str, value: Value) {
        if self.events.len() >= 32 {
            self.events.pop_front();
        }
        self.events
            .push_back(json!({"operation":name,"value":value}));
    }
}
impl Devices for Simulated {
    fn begin(&mut self) -> Result<()> {
        self.record("begin_control", Value::Null);
        Ok(())
    }
    fn apply(&mut self, t: &pb::Targets) -> Result<()> {
        self.record("targets", serde_json::to_value(t)?);
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.record("stop", Value::Null);
        Ok(())
    }
    fn stop_chassis(&mut self) -> Result<()> {
        self.record("stop_chassis", Value::Null);
        Ok(())
    }
    fn zero(&mut self, mask: u8) -> Result<()> {
        self.record("zero_neck", json!(mask));
        Ok(())
    }
    fn chassis(&mut self, t: &pb::ChassisTarget) -> Result<()> {
        self.record("chassis", json!({"mode":t.mode,"data":chassis_payload(t)}));
        Ok(())
    }
    fn poll(&mut self) -> Result<()> {
        Ok(())
    }
    fn snapshot(&self) -> Value {
        json!({"feedback_kind":"simulated","events":self.events})
    }
}

pub fn chassis_payload(t: &pb::ChassisTarget) -> [u8; 8] {
    let mut data = [0; 8];
    for (i, v) in [t.speed_m_s, t.yaw_rate_rad_s, 0.0, t.steering_rad]
        .iter()
        .enumerate()
    {
        data[i * 2..i * 2 + 2].copy_from_slice(&((*v * 1000.0).round() as i16).to_be_bytes());
    }
    data
}
fn u16be(d: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([d[i], d[i + 1]])
}
fn i16be(d: &[u8], i: usize) -> i16 {
    u16be(d, i) as i16
}
fn i32be(d: &[u8], i: usize) -> i32 {
    i32::from_be_bytes(d[i..i + 4].try_into().unwrap())
}
pub fn decode_chassis(id: u32, d: &[u8]) -> Option<(String, Value)> {
    if id == 0x291 && d.len() == 3 {
        return Some((
            "mode".into(),
            json!({"mode":d[0],"switching":d[1]!=0,"drive_mode":d[2]}),
        ));
    }
    if id == 0x362 && d.len() == 4 {
        return Some((
            "battery_alarm".into(),
            json!({"alarm":&d[..2],"warning":&d[2..]}),
        ));
    }
    if d.len() != 8 {
        return None;
    }
    let (key, v) = match id {
        0x211 => (
            "system".into(),
            json!({"state":d[0],"control_mode":d[1],"voltage_v":u16be(d,2) as f64/10.0,"fault_bits":i32be(d,4) as u32}),
        ),
        0x221 => (
            "motion".into(),
            json!({"speed_m_s":i16be(d,0) as f64/1000.0,"yaw_rate_rad_s":i16be(d,2) as f64/1000.0,"steering_raw_rad":i16be(d,6) as f64/1000.0}),
        ),
        0x271 | 0x281 => (
            if id == 0x271 {
                "wheel_angles"
            } else {
                "wheel_speeds"
            }
            .into(),
            json!({"values":(0..4).map(|i|i16be(d,i*2) as f64/1000.0).collect::<Vec<_>>()}),
        ),
        0x311 | 0x312 => (
            if id == 0x311 {
                "front_odometry"
            } else {
                "rear_odometry"
            }
            .into(),
            json!({"left_m":i32be(d,0) as f64/1000.0,"right_m":i32be(d,4) as f64/1000.0}),
        ),
        0x241 => (
            "remote".into(),
            json!({"swa":d[0]&3,"swb":(d[0]>>2)&3,"swc":(d[0]>>4)&3,"swd":(d[0]>>6)&3,"axes":d[1..6].iter().map(|v|*v as i8).collect::<Vec<_>>()}),
        ),
        0x361 => (
            "battery".into(),
            json!({"soc":d[0],"soh":d[1],"voltage_v":u16be(d,2) as f64/10.0,"current_a":i16be(d,4) as f64/10.0,"temperature_c":i16be(d,6) as f64/10.0}),
        ),
        0x251..=0x258 => (
            format!("motor_{}_fast", id - 0x250),
            json!({"rpm":i16be(d,0),"current_a":i16be(d,2) as f64/10.0,"position_counts":i32be(d,4)}),
        ),
        0x261..=0x268 => (
            format!("motor_{}_slow", id - 0x260),
            json!({"voltage_v":u16be(d,0) as f64/10.0,"driver_temperature_c":i16be(d,2),"motor_temperature_c":d[4] as i8,"status":d[5]}),
        ),
        _ => return None,
    };
    Some((key, v))
}

pub struct Servo {
    port: Box<dyn SerialPort>,
    cfg: Arm,
    pending: Option<(usize, Instant, Vec<u8>)>,
}
impl Servo {
    fn open(cfg: Arm) -> Result<Self> {
        let builder = serialport::new(&cfg.port, cfg.baudrate)
            .timeout(Duration::from_millis(5))
            .flow_control(serialport::FlowControl::None);
        #[cfg(target_os = "linux")]
        let mut port: Box<dyn SerialPort> = {
            use std::os::fd::AsRawFd;
            let p = builder.open_native()?;
            if cfg.direction_mode == "kernel" {
                #[repr(C)]
                struct Rs485 {
                    flags: u32,
                    before: u32,
                    after: u32,
                    padding: [u32; 5],
                }
                let mut settings = Rs485 {
                    flags: 1 | if cfg.rts_tx { 2 } else { 4 },
                    before: 0,
                    after: 0,
                    padding: [0; 5],
                };
                // 内核必须接管发送末位到 RTS 释放，不能再同时手动翻转。
                let r = unsafe { libc::ioctl(p.as_raw_fd(), 0x542f, &mut settings) };
                ensure!(
                    r == 0,
                    "UART 驱动不支持 RS485 RTS：{}",
                    std::io::Error::last_os_error()
                );
            }
            Box::new(p)
        };
        #[cfg(not(target_os = "linux"))]
        let mut port = {
            ensure!(cfg.direction_mode == "manual", "内核 RTS 模式需要 Linux");
            builder.open()?
        };
        if cfg.direction_mode == "manual" {
            port.write_request_to_send(!cfg.rts_tx)?;
        }
        Ok(Self {
            port,
            cfg,
            pending: None,
        })
    }
    fn send(&mut self, wire: &[u8]) -> Result<()> {
        let manual = self.cfg.direction_mode == "manual";
        let result: Result<()> = (|| {
            if manual {
                self.port.write_request_to_send(self.cfg.rts_tx)?;
            }
            self.port.write_all(wire)?;
            self.port.flush()?;
            Ok(())
        })();
        if manual {
            let release = self.port.write_request_to_send(!self.cfg.rts_tx);
            result?;
            release?;
        } else {
            result?;
        }
        Ok(())
    }
    fn target(&mut self, t: &pb::ArmTarget) -> Result<()> {
        for i in 0..4 {
            let j = &self.cfg.joints[i];
            self.send(
                format!(
                    "#{:03}P{:04}T{:04}!",
                    j.id,
                    j.pwm(t.joint_rad[i]),
                    t.duration_ms
                )
                .as_bytes(),
            )?;
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for i in 0..4 {
            if let Err(e) = self.send(format!("#{:03}PDST!", self.cfg.joints[i].id).as_bytes()) {
                errors.push(e.to_string());
            }
        }
        self.pending = None;
        ensure!(errors.is_empty(), "{}", errors.join("；"));
        Ok(())
    }
    fn query(&mut self, index: usize) -> Result<()> {
        self.port.clear(serialport::ClearBuffer::Input)?;
        self.send(format!("#{:03}PRAD!", self.cfg.joints[index].id).as_bytes())?;
        self.pending = Some((
            index,
            Instant::now() + Duration::from_millis(self.cfg.timeout_ms),
            Vec::new(),
        ));
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<(usize, Value)>> {
        let Some((index, end, buf)) = &mut self.pending else {
            return Ok(None);
        };
        let mut data = [0; 256];
        let n = self.port.bytes_to_read()?.min(256) as usize;
        let n = if n > 0 {
            self.port.read(&mut data[..n])?
        } else {
            0
        };
        let joint = &self.cfg.joints[*index];
        let mut found = None;
        for &b in &data[..n] {
            if b == b'#' {
                buf.clear();
            }
            if buf.len() >= 256 {
                buf.clear();
            }
            if matches!(b, b'!' | b'\r' | b'\n') {
                let text = String::from_utf8_lossy(buf);
                let prefix = format!("#{:03}P", joint.id);
                if let Some(value) = text.strip_prefix(&prefix) {
                    if let Some(err) = value.strip_prefix("ERR:") {
                        found = Some(json!({"error":err}));
                    } else if let Ok(pwm) = value.parse::<u16>()
                        && (500..=2500).contains(&pwm)
                    {
                        found = Some(json!({"id":joint.id,"pwm":pwm,"error":"",
                            "position_rad":(pwm as f64-joint.center_pwm as f64)/(joint.direction as f64*joint.pwm_per_rad)}));
                    }
                }
                buf.clear();
            } else {
                buf.push(b);
            }
        }
        if found.is_none() && Instant::now() >= *end {
            found = Some(json!({"error":"位置反馈超时"}));
        }
        if let Some(mut value) = found {
            value["feedback_time"] =
                json!(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64());
            let i = *index;
            self.pending = None;
            return Ok(Some((i, value)));
        }
        Ok(None)
    }
}
impl Drop for Servo {
    fn drop(&mut self) {
        if self.cfg.direction_mode == "manual" {
            let _ = self.port.write_request_to_send(!self.cfg.rts_tx);
        }
    }
}

pub struct Hardware {
    cfg: Config,
    board: Option<Bridge>,
    arms: [Option<Servo>; 2],
    eye: Option<ep::eye_control_client::EyeControlClient<Channel>>,
    rt: tokio::runtime::Handle,
    state: Value,
    times: HashMap<String, Instant>,
    requested_mode: i32,
    eye_cfg: ep::Config,
    eye_pose: Option<ep::Pose>,
    eye_revision: u64,
    next_neck: Instant,
    next_arm: Instant,
    next_eye: Instant,
    poll_index: usize,
}
impl Hardware {
    pub fn connect(cfg: Config, rt: tokio::runtime::Handle) -> Result<Self> {
        let eye_cfg = serde_json::from_value(cfg.eyes.settings.clone())?;
        let mut h = Self {
            cfg,
            board: None,
            arms: [None, None],
            eye: None,
            rt,
            state: json!({"chassis":{},"neck":{},"left_arm":{},"right_arm":{},"eyes":{}}),
            times: HashMap::new(),
            requested_mode: 3,
            eye_cfg,
            eye_pose: None,
            eye_revision: 0,
            next_neck: Instant::now(),
            next_arm: Instant::now(),
            next_eye: Instant::now(),
            poll_index: 0,
        };
        if h.cfg.neck.enabled || h.cfg.chassis.enabled {
            let mut b = Bridge::open(&h.cfg.h7)?;
            let (_, info) = b.request(1, 0, vec![])?;
            ensure!(info.len() >= 8, "H7 INFO 长度错误");
            ensure!(
                !h.cfg.neck.enabled || info[..3] >= [1, 1, 0][..],
                "H7 需要 v1.1.0 或更新固件"
            );
            h.state["h7"] = json!({"firmware":format!("{}.{}.{}",info[0],info[1],info[2]),"board":String::from_utf8_lossy(&info[8..])});
            h.board = Some(b);
            let b = h.board.as_mut().unwrap();
            if h.cfg.neck.enabled {
                b.motor_stop(3)?;
                b.configure(1, 1000000)?;
                b.motor_tune(1, &h.cfg.neck.yaw.tuning)?;
                b.motor_tune(2, &h.cfg.neck.pitch.tuning)?;
            }
            if h.cfg.chassis.enabled {
                b.configure(h.cfg.chassis.channel, 500000)?;
            }
        }
        for (i, a) in [h.cfg.left_arm.clone(), h.cfg.right_arm.clone()]
            .into_iter()
            .enumerate()
        {
            if a.enabled {
                h.arms[i] = Some(Servo::open(a)?);
            }
        }
        if h.cfg.eyes.enabled {
            let target = h.cfg.eyes.target.clone();
            let channel = h.rt.block_on(async {
                if let Some(path) = target.strip_prefix("unix:") {
                    #[cfg(unix)]
                    {
                        let path = path.to_string();
                        Endpoint::try_from("http://[::]:50051")?
                            .connect_with_connector(tower::service_fn(move |_| {
                                let path = path.clone();
                                async move {
                                    tokio::net::UnixStream::connect(path)
                                        .await
                                        .map(hyper_util::rt::TokioIo::new)
                                }
                            }))
                            .await
                            .map_err(anyhow::Error::from)
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = path;
                        anyhow::bail!(
                            "Windows 实机眼屏 AF_UNIX 连接请使用 Linux 部署；模拟模式无需连接"
                        );
                    }
                } else {
                    Endpoint::from_shared(format!(
                        "http://{}",
                        target.trim_start_matches("http://")
                    ))?
                    .connect()
                    .await
                    .map_err(anyhow::Error::from)
                }
            })?;
            let mut eye = ep::eye_control_client::EyeControlClient::new(channel);
            h.eye_revision =
                h.rt.block_on(async {
                    tokio::time::timeout(
                        Duration::from_millis(300),
                        eye.configure(h.eye_cfg.clone()),
                    )
                    .await
                })??
                .into_inner()
                .revision;
            h.eye = Some(eye);
        }
        h.begin()?;
        h.stop()?;
        h.poll()?;
        Ok(h)
    }
    fn board(&mut self) -> Result<&mut Bridge> {
        self.board
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("H7 未连接"))
    }
    fn can(&mut self, id: u32, data: &[u8]) -> Result<()> {
        let ch = self.cfg.chassis.channel;
        self.board()?.send(ch, id, data)
    }
    fn neck_status(&mut self) -> Result<()> {
        let (_, data) = self.board()?.request(0x11, 1, vec![])?;
        ensure!(data.len() == 68, "电机状态长度错误");
        let states = ["停止", "等待使能反馈", "位置控制", "故障停止"];
        let faults = [
            "无",
            "反馈超时或失去使能",
            "驱动故障",
            "总线不可用",
            "发送阻塞",
            "CDC 断开",
            "控制周期超时",
            "位置参考失效",
            "温度超限",
        ];
        for (i, (key, axis, unit)) in [
            ("gm6020", &self.cfg.neck.yaw, "A"),
            ("dm4310", &self.cfg.neck.pitch, "Nm"),
        ]
        .into_iter()
        .enumerate()
        {
            let d = &data[i * 32..i * 32 + 32];
            ensure!(
                d[0] == i as u8 + 1
                    && (d[1] as usize) < states.len()
                    && (d[2] as usize) < faults.len(),
                "电机状态字段错误"
            );
            let f = |offset: usize| {
                f32::from_le_bytes(d[offset..offset + 4].try_into().unwrap()) as f64
            };
            let u = |offset: usize| u32::from_le_bytes(d[offset..offset + 4].try_into().unwrap());
            self.state["neck"][key] = json!({"state":states[d[1] as usize],"fault":faults[d[2] as usize],"temperature_c":d[3],
                "position_rad":f(4),"joint_position_rad":(f(4)-axis.offset_rad)/axis.direction as f64,
                "position_deg":f(4).to_degrees(),"velocity_rad_s":f(8),"effort":f(12),"target_rad":f(16),"output":f(20),
                "effort_unit":unit,"feedback_age_ms":if u(24)==u32::MAX {None} else {Some(u(24))},"tx_failures":u(28)});
        }
        self.state["neck"]["missed_control_ticks"] =
            json!(u32::from_le_bytes(data[64..].try_into().unwrap()));
        Ok(())
    }
}
impl Devices for Hardware {
    fn begin(&mut self) -> Result<()> {
        if self.cfg.chassis.enabled {
            self.can(0x421, &[1])?;
        }
        Ok(())
    }
    fn apply(&mut self, t: &pb::Targets) -> Result<()> {
        if let Some(n) = &t.neck {
            let yaw = n
                .yaw_rad
                .map(|v| v * self.cfg.neck.yaw.direction as f64 + self.cfg.neck.yaw.offset_rad);
            let pitch = n
                .pitch_rad
                .map(|v| v * self.cfg.neck.pitch.direction as f64 + self.cfg.neck.pitch.offset_rad);
            self.board()?.motor_position(yaw, pitch)?;
        }
        for (i, t) in [&t.left_arm, &t.right_arm].into_iter().enumerate() {
            if let Some(t) = t {
                self.arms[i]
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("手臂未连接"))?
                    .target(t)?;
            }
        }
        if let Some(p) = &t.eyes {
            let eye = self
                .eye
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("眼屏未连接"))?;
            self.eye_revision = self
                .rt
                .block_on(async {
                    tokio::time::timeout(Duration::from_millis(300), eye.set_pose(*p)).await
                })??
                .into_inner()
                .revision;
            self.eye_cfg.mode = Some(ep::Mode::External as i32);
            self.eye_pose = Some(*p);
        }
        if let Some(b) = &t.blink {
            let eye = self
                .eye
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("眼屏未连接"))?;
            self.rt.block_on(async {
                tokio::time::timeout(Duration::from_millis(300), eye.trigger_blink(*b)).await
            })??;
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        if self.cfg.chassis.enabled
            && self.board.is_some()
            && let Err(e) = self.stop_chassis()
        {
            errors.push(e.to_string());
        }
        if self.cfg.neck.enabled
            && self.board.is_some()
            && let Err(e) = self.board()?.motor_stop(3)
        {
            errors.push(e.to_string());
        }
        for a in self.arms.iter_mut().flatten() {
            if let Err(e) = a.stop() {
                errors.push(e.to_string());
            }
        }
        ensure!(errors.is_empty(), "停止失败：{}", errors.join("；"));
        Ok(())
    }
    fn stop_chassis(&mut self) -> Result<()> {
        self.can(0x111, &[0; 8])?;
        self.can(0x141, &[3])?;
        self.requested_mode = 3;
        self.times.remove("mode");
        Ok(())
    }
    fn zero(&mut self, mask: u8) -> Result<()> {
        self.board()?.motor_zero(mask)
    }
    fn chassis(&mut self, t: &pb::ChassisTarget) -> Result<()> {
        if self.requested_mode != t.mode {
            self.can(0x111, &[0; 8])?;
            self.can(0x141, &[t.mode as u8])?;
            self.requested_mode = t.mode;
            self.times.remove("mode");
        }
        let sys = &self.state["chassis"]["system"];
        let mode = &self.state["chassis"]["mode"];
        let fresh = ["system", "mode"].iter().all(|k| {
            self.times.get(*k).is_some_and(|t| {
                t.elapsed() < Duration::from_millis(self.cfg.chassis.feedback_timeout_ms)
            })
        });
        let ready = fresh
            && sys["state"] == 0
            && sys["fault_bits"] == 0
            && sys["control_mode"] == 1
            && mode["mode"] == t.mode
            && mode["switching"] == false;
        self.can(0x111, &if ready { chassis_payload(t) } else { [0; 8] })?;
        self.state["chassis"]["ready"] = json!(ready);
        Ok(())
    }
    fn poll(&mut self) -> Result<()> {
        let now = Instant::now();
        if let Some(b) = &mut self.board {
            b.poll()?;
            for _ in 0..256 {
                let Some(p) = b.frames.pop_front() else { break };
                if p.channel != self.cfg.chassis.channel || p.flags != 0 || p.payload.len() < 9 {
                    continue;
                }
                let id = u32::from_le_bytes(p.payload[..4].try_into().unwrap());
                if let Some((key, value)) = decode_chassis(id, &p.payload[9..]) {
                    self.state["chassis"][&key] = value;
                    self.times.insert(key, now);
                }
            }
            self.state["chassis"]["feedback_age_ms"] = json!(
                self.times
                    .iter()
                    .map(|(k, t)| (k.clone(), t.elapsed().as_millis()))
                    .collect::<HashMap<_, _>>()
            );
            self.state["chassis"]["host_rx_dropped"] = json!(b.dropped);
        }
        if self.cfg.neck.enabled && now >= self.next_neck {
            self.neck_status()?;
            self.next_neck = now + Duration::from_millis(200);
        }
        for (i, a) in self.arms.iter_mut().enumerate() {
            if let Some(a) = a
                && let Some((index, value)) = a.poll()?
            {
                self.state[if i == 0 { "left_arm" } else { "right_arm" }][index.to_string()] =
                    value;
            }
        }
        if now >= self.next_arm && self.arms.iter().flatten().all(|a| a.pending.is_none()) {
            let items: Vec<_> = self
                .arms
                .iter()
                .enumerate()
                .filter(|(_, a)| a.is_some())
                .flat_map(|(i, _)| (0..4).map(move |j| (i, j)))
                .collect();
            if !items.is_empty() {
                let (i, j) = items[self.poll_index % items.len()];
                self.arms[i].as_mut().unwrap().query(j)?;
                self.poll_index += 1;
            }
            self.next_arm = now + Duration::from_millis(100);
        }
        if let Some(eye) = &mut self.eye
            && now >= self.next_eye
        {
            self.rt.block_on(async {
                let s =
                    tokio::time::timeout(Duration::from_millis(300), eye.get_state(ep::Empty {}))
                        .await??
                        .into_inner();
                if s.config.as_ref() != Some(&self.eye_cfg) || s.revision < self.eye_revision {
                    tokio::time::timeout(
                        Duration::from_millis(300),
                        eye.configure(self.eye_cfg.clone()),
                    )
                    .await??;
                    if let Some(p) = &self.eye_pose {
                        self.eye_revision =
                            tokio::time::timeout(Duration::from_millis(300), eye.set_pose(*p))
                                .await??
                                .into_inner()
                                .revision;
                    }
                }
                self.state["eyes"] = serde_json::to_value(s)?;
                Ok::<_, anyhow::Error>(())
            })?;
            self.next_eye = now + Duration::from_millis(500);
        }
        Ok(())
    }
    fn snapshot(&self) -> Value {
        self.state.clone()
    }
}
impl Drop for Hardware {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
