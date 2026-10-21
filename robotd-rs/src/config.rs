use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub metadata: serde_json::Map<String, Value>,
    pub h7: H7,
    pub chassis: Chassis,
    pub neck: Neck,
    pub left_arm: Arm,
    pub right_arm: Arm,
    pub eyes: Eyes,
    pub audio: Audio,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct H7 {
    pub port: String,
    pub timeout_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chassis {
    pub enabled: bool,
    pub channel: u8,
    pub bitrate: u32,
    pub max_speed_m_s: f64,
    pub max_yaw_rate_rad_s: f64,
    pub max_steering_rad: f64,
    pub feedback_timeout_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Neck {
    pub enabled: bool,
    pub channel: u8,
    pub yaw: Axis,
    pub pitch: Axis,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub min_rad: f64,
    pub max_rad: f64,
    pub offset_rad: f64,
    pub direction: i8,
    pub tuning: Tuning,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    pub position_kp: f64,
    pub velocity_kp: f64,
    pub velocity_ki: f64,
    pub max_velocity: f64,
    pub max_output: f64,
    pub max_position: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    pub enabled: bool,
    pub port: String,
    pub baudrate: u32,
    pub rts_tx: bool,
    pub direction_mode: String,
    pub timeout_ms: u64,
    pub joints: [Joint; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Joint {
    pub id: u8,
    pub min_rad: f64,
    pub max_rad: f64,
    pub center_pwm: u16,
    pub pwm_per_rad: f64,
    pub direction: i8,
}
impl Joint {
    pub fn pwm(&self, angle: f64) -> u16 {
        (self.center_pwm as f64 + self.direction as f64 * angle * self.pwm_per_rad).round() as u16
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Eyes {
    pub enabled: bool,
    pub target: String,
    pub settings: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audio {
    pub enabled: bool,
    pub device: String,
    pub prebuffer_ms: u32,
    pub rebuffer_ms: u32,
    pub max_buffer_ms: u32,
    pub device_buffer_ms: u32,
}

pub fn range(v: f64, lo: f64, hi: f64, name: &str) -> Result<()> {
    ensure!(
        v.is_finite() && v >= lo && v <= hi,
        "{name} 必须在 [{lo}, {hi}] 内且有限"
    );
    Ok(())
}

fn normalize(v: &mut Value) {
    match v {
        Value::Object(o) => o.values_mut().for_each(normalize),
        Value::Array(a) => a.iter_mut().for_each(normalize),
        Value::Number(n) if n.is_f64() => {
            if let Some(f) = n.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 9e15) {
                *v = Value::from(f as i64);
            }
        }
        _ => {}
    }
}

impl Config {
    pub fn defaults() -> Self {
        Self::parse(serde_json::from_str(include_str!("../../robotd/default-config.json")).unwrap())
            .unwrap()
    }
    pub fn parse(mut v: Value) -> Result<Self> {
        normalize(&mut v);
        if v["schema_version"] == 1 {
            ensure!(
                v.get("audio").is_none(),
                "v1 配置不包含 audio；带音频配置请使用 schema_version=2"
            );
            v["schema_version"] = Value::from(2);
            v["audio"] = serde_json::from_str::<Value>(include_str!(
                "../../robotd/default-config.json"
            ))?["audio"]
                .clone();
        }
        let c: Self = serde_json::from_value(v)?;
        c.validate()?;
        Ok(c)
    }
    pub fn value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 2, "仅支持配置 schema_version=1/2");
        ensure!(
            !self.h7.port.is_empty() && (50..=300).contains(&self.h7.timeout_ms),
            "H7 配置无效"
        );
        let c = &self.chassis;
        ensure!(
            (2..=3).contains(&c.channel) && c.bitrate == 500000,
            "底盘要求 CAN2/3、500 kbps"
        );
        range(c.max_speed_m_s, 0.001, 1.5, "底盘速度")?;
        range(c.max_yaw_rate_rad_s, 0.001, 3.259, "底盘自旋速度")?;
        range(c.max_steering_rad, 0.001, 1.571, "底盘转角")?;
        ensure!(
            (100..=1000).contains(&c.feedback_timeout_ms),
            "底盘反馈超时无效"
        );
        ensure!(self.neck.channel == 1, "当前 H7 固件颈部固定使用 CAN1");
        for (a, output) in [(&self.neck.yaw, 3.0), (&self.neck.pitch, 10.0)] {
            range(a.min_rad, -12.5, 12.5, "颈部下限")?;
            range(a.max_rad, a.min_rad + 0.001, 12.5, "颈部上限")?;
            range(a.offset_rad, -12.5, 12.5, "颈部偏置")?;
            ensure!([-1, 1].contains(&a.direction), "颈部方向必须为 ±1");
            for x in [
                a.tuning.position_kp,
                a.tuning.velocity_kp,
                a.tuning.velocity_ki,
            ] {
                range(x, 0.0, 100.0, "控制增益")?;
            }
            range(a.tuning.max_velocity, 0.001, 30.0, "颈部速度上限")?;
            range(a.tuning.max_output, 0.001, output, "颈部输出上限")?;
            range(a.tuning.max_position, 0.001, 12.5, "颈部位置上限")?;
            for x in [a.min_rad, a.max_rad] {
                ensure!(
                    (x * a.direction as f64 + a.offset_rad).abs() <= a.tuning.max_position,
                    "颈部换算后超过 H7 位置限制"
                );
            }
        }
        let mut ports = vec![&self.h7.port];
        for arm in [&self.left_arm, &self.right_arm] {
            ensure!(
                !arm.port.is_empty() && (9600..=1000000).contains(&arm.baudrate),
                "手臂串口配置无效"
            );
            ensure!((20..=200).contains(&arm.timeout_ms), "舵机反馈超时无效");
            ensure!(
                ["manual", "kernel"].contains(&arm.direction_mode.as_str()),
                "RTS 模式无效"
            );
            if arm.enabled {
                ports.push(&arm.port);
            }
            let mut ids = Vec::new();
            for j in &arm.joints {
                ensure!(
                    j.id <= 253 && !ids.contains(&j.id),
                    "同一总线舵机 ID 重复或无效"
                );
                ids.push(j.id);
                ensure!(
                    (500..=2500).contains(&j.center_pwm) && [-1, 1].contains(&j.direction),
                    "舵机中心或方向无效"
                );
                range(j.pwm_per_rad, 1.0, 10000.0, "PWM 换算")?;
                range(
                    j.min_rad,
                    -std::f64::consts::PI,
                    std::f64::consts::PI,
                    "关节下限",
                )?;
                range(
                    j.max_rad,
                    j.min_rad + 0.001,
                    std::f64::consts::PI,
                    "关节上限",
                )?;
                for x in [j.min_rad, j.max_rad] {
                    range(
                        j.center_pwm as f64 + j.direction as f64 * x * j.pwm_per_rad,
                        500.0,
                        2500.0,
                        "关节 PWM 限位",
                    )?;
                }
            }
        }
        for (i, p) in ports.iter().enumerate() {
            ensure!(!ports[..i].contains(p), "设备串口不能重复");
        }
        ensure!(!self.eyes.target.is_empty(), "眼屏地址不能为空");
        validate_eye_config(&self.eyes.settings)?;
        let a = &self.audio;
        ensure!(
            !a.device.is_empty() && !a.device.contains('\0'),
            "ALSA 设备名无效"
        );
        ensure!(
            (10..=100).contains(&a.device_buffer_ms),
            "ALSA 缓冲应为 10..100 ms"
        );
        ensure!(
            (10..=200).contains(&a.prebuffer_ms) && a.prebuffer_ms <= a.max_buffer_ms,
            "音频预缓冲必须在 10..200 ms 且不大于队列上限"
        );
        ensure!(
            (10..=a.max_buffer_ms).contains(&a.rebuffer_ms),
            "音频恢复缓冲无效"
        );
        ensure!(
            a.max_buffer_ms + a.device_buffer_ms <= 200,
            "软件与 ALSA 缓冲配置之和不能超过 200 ms"
        );
        Ok(())
    }
}

pub fn validate_eye_config(v: &Value) -> Result<()> {
    let _: crate::eyes::v1::Config = serde_json::from_value(v.clone())?;
    let template: Value = serde_json::from_str(include_str!("../../robotd/default-config.json"))?;
    fn shape(v: &Value, t: &Value) -> Result<()> {
        if let Some(o) = t.as_object() {
            let Some(actual) = v.as_object() else {
                bail!("眼屏配置对象无效");
            };
            ensure!(actual.len() == o.len(), "眼屏配置字段不完整");
            for (k, value) in o {
                shape(
                    actual
                        .get(k)
                        .ok_or_else(|| anyhow::anyhow!("缺少眼屏字段 {k}"))?,
                    value,
                )?;
            }
        } else if t.is_boolean() {
            ensure!(v.is_boolean(), "眼屏布尔字段无效");
        }
        Ok(())
    }
    shape(v, &template["eyes"]["settings"])?;
    for (path, lo, hi) in [
        ("scale", 0.5, 1.5),
        ("spacing", 0.38, 0.7),
        ("iris_size", 0.1, 0.22),
        ("pupil", 0.2, 0.65),
        ("tilt", -0.2, 0.3),
        ("glow", 0.0, 1.0),
        ("expression", -1.0, 1.0),
        ("response", 3.0, 30.0),
        ("blink_interval", 0.5, 20.0),
        ("blink_duration", 0.08, 1.0),
        ("blink_variation", 0.0, 0.8),
        ("jelly/strength", 0.0, 1.0),
        ("jelly/frequency", 1.0, 12.0),
        ("jelly/damping", 0.15, 1.5),
        ("simulator/target_x", -1.0, 1.0),
        ("simulator/target_y", -1.0, 1.0),
        ("simulator/motion_amplitude", 0.0, 0.3),
        ("simulator/motion_speed", 0.1, 8.0),
        ("simulator/convergence", 0.0, 0.3),
        ("simulator/left_openness", 0.0, 1.4),
        ("simulator/right_openness", 0.0, 1.4),
        ("simulator/left_brow", -1.0, 1.0),
        ("simulator/right_brow", -1.0, 1.0),
        ("simulator/roll", -0.8, 0.8),
        ("art/half_width", 0.2, 0.36),
        ("art/upper_height", 0.08, 0.3),
        ("art/lower_height", 0.06, 0.28),
        ("art/roundness", 0.5, 1.5),
        ("art/iris_aspect", 0.6, 1.2),
        ("art/brow_height", 0.22, 0.5),
        ("art/lid_texture_height", 0.04, 0.25),
        ("art/shadow_strength", 0.0, 0.5),
        ("iris/r", 0.0, 1.0),
        ("iris/g", 0.0, 1.0),
        ("iris/b", 0.0, 1.0),
        ("background/r", 0.0, 1.0),
        ("background/g", 0.0, 1.0),
        ("background/b", 0.0, 1.0),
    ] {
        let x = v
            .pointer(&format!("/{path}"))
            .and_then(Value::as_f64)
            .ok_or_else(|| anyhow::anyhow!("眼屏字段 {path} 必须为数值"))?;
        range(x, lo, hi, path)?;
    }
    let port = v["tracking_port"].as_u64().unwrap_or(0);
    ensure!((1024..=65535).contains(&port), "tracking_port 无效");
    let ip = v["phone_ip"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("phone_ip 必须为字符串"))?;
    ensure!(
        ip.is_empty() || ip.parse::<std::net::Ipv4Addr>().is_ok(),
        "phone_ip 无效"
    );
    Ok(())
}
