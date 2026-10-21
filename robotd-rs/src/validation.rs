use crate::{
    config::{Config, range},
    pb,
};
use anyhow::{Result, ensure};

pub fn targets(t: &pb::Targets, cfg: &Config) -> Result<()> {
    ensure!(
        t.chassis.is_some()
            || t.neck.is_some()
            || t.left_arm.is_some()
            || t.right_arm.is_some()
            || t.eyes.is_some()
            || t.blink.is_some(),
        "目标不能为空"
    );
    if let Some(c) = &t.chassis {
        ensure!(cfg.chassis.enabled, "底盘已禁用");
        ensure!(
            (0..=3).contains(&c.mode) && (100..=1000).contains(&c.ttl_ms),
            "底盘模式或 TTL 无效"
        );
        range(
            c.speed_m_s,
            -cfg.chassis.max_speed_m_s,
            cfg.chassis.max_speed_m_s,
            "底盘速度",
        )?;
        range(
            c.yaw_rate_rad_s,
            -cfg.chassis.max_yaw_rate_rad_s,
            cfg.chassis.max_yaw_rate_rad_s,
            "自旋速度",
        )?;
        let limit = cfg
            .chassis
            .max_steering_rad
            .min(if c.mode == 0 { 0.698 } else { 1.571 });
        range(c.steering_rad, -limit, limit, "转角")?;
        ensure!(
            c.steering_rad.abs() <= 20f64.to_radians() || c.speed_m_s.abs() <= 0.525,
            "大转角速度超过 0.525 m/s"
        );
        ensure!(
            c.mode > 1 || c.yaw_rate_rad_s == 0.0,
            "阿克曼/斜移只接受速度和转角"
        );
        ensure!(
            c.mode < 2 || (c.speed_m_s == 0.0 && c.steering_rad == 0.0),
            "自旋/驻车不能指定速度和转角"
        );
        ensure!(c.mode != 3 || c.yaw_rate_rad_s == 0.0, "驻车模式不能运动");
    }
    if let Some(n) = &t.neck {
        ensure!(cfg.neck.enabled, "颈部已禁用");
        ensure!(
            n.yaw_rad.is_some() || n.pitch_rad.is_some(),
            "颈部目标不能为空"
        );
        for (x, a) in [(n.yaw_rad, &cfg.neck.yaw), (n.pitch_rad, &cfg.neck.pitch)] {
            if let Some(x) = x {
                range(x, a.min_rad, a.max_rad, "颈部角度")?;
            }
        }
    }
    for (t, a) in [(&t.left_arm, &cfg.left_arm), (&t.right_arm, &cfg.right_arm)] {
        if let Some(t) = t {
            ensure!(a.enabled, "手臂已禁用");
            ensure!(
                t.joint_rad.len() == 4 && (1..=9999).contains(&t.duration_ms),
                "手臂需要 4 个关节角和有效时间"
            );
            for (x, j) in t.joint_rad.iter().zip(&a.joints) {
                range(*x, j.min_rad, j.max_rad, "关节目标")?;
            }
        }
    }
    if let Some(p) = &t.eyes {
        ensure!(cfg.eyes.enabled, "眼屏已禁用");
        for e in [&p.left, &p.right] {
            let e = e
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("必须同时提供左右眼"))?;
            range(e.gaze_x as f64, -1.0, 1.0, "gaze_x")?;
            range(e.gaze_y as f64, -1.0, 1.0, "gaze_y")?;
            range(e.openness as f64, 0.0, 1.4f32 as f64, "openness")?;
            range(e.brow as f64, -1.0, 1.0, "brow")?;
        }
        range(p.roll as f64, -0.8f32 as f64, 0.8f32 as f64, "roll")?;
    }
    if let Some(b) = &t.blink {
        ensure!(cfg.eyes.enabled, "眼屏已禁用");
        if let Some(d) = b.duration {
            range(d as f64, 0.08f32 as f64, 1.0, "眨眼时长")?;
        }
    }
    Ok(())
}
pub fn action(a: &pb::Action, cfg: &Config) -> Result<()> {
    ensure!(
        !a.name.is_empty()
            && a.name.chars().count() <= 64
            && a.name
                .chars()
                .all(|x| x.is_alphanumeric() || x == '_' || x == '-'),
        "动作名称无效"
    );
    ensure!(
        (1..=600000).contains(&a.duration_ms) && (1..=1000).contains(&a.steps.len()),
        "动作时长或时间点数量无效"
    );
    let mut last = None;
    for s in &a.steps {
        ensure!(
            s.at_ms < a.duration_ms && last.is_none_or(|x| s.at_ms > x),
            "动作时间点必须递增且早于结束时间"
        );
        targets(
            s.targets
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("缺少动作目标"))?,
            cfg,
        )?;
        last = Some(s.at_ms);
    }
    Ok(())
}
