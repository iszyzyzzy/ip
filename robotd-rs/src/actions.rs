use crate::{config::Config, pb, validation};
use anyhow::{Context, Result, ensure};
use pb::action::Definition;
use serde_json::Value;

fn preset_step(preset: &pb::ActionPreset, move_ms: u32) -> Result<pb::Targets> {
    ensure!((1..=9999).contains(&move_ms), "运动时间须在 1～9999 毫秒内");
    let mut targets = preset.targets.clone().context("缺少预设姿态")?;
    ensure!(
        targets.chassis.is_none() && targets.servo.is_none() && targets.blink.is_none(),
        "预设只保存关节姿态和眼屏状态"
    );
    for arm in [&mut targets.left_arm, &mut targets.right_arm]
        .into_iter()
        .flatten()
    {
        arm.duration_ms = move_ms;
    }
    Ok(targets)
}

// 执行前解析引用；运行中的时间线不再读取可编辑的预设库。
pub fn compile(
    action: &pb::Action,
    cfg: &Config,
    mut lookup: impl FnMut(&str) -> Result<pb::Action>,
) -> Result<pb::Action> {
    let Some(definition) = &action.definition else {
        validation::action(action, cfg)?;
        return Ok(action.clone());
    };
    ensure!(
        action.steps.is_empty() && action.duration_ms == 0,
        "预设或序列不能同时包含时间点动作"
    );
    let mut plan = pb::Action {
        name: action.name.clone(),
        ..Default::default()
    };
    let mut append = |preset: &pb::ActionPreset, move_ms: u32, hold_ms: u32| -> Result<()> {
        plan.steps.push(pb::ActionStep {
            at_ms: plan.duration_ms,
            targets: Some(preset_step(preset, move_ms)?),
        });
        plan.duration_ms = plan
            .duration_ms
            .checked_add(move_ms)
            .and_then(|t| t.checked_add(hold_ms))
            .context("动作总时长超限")?;
        Ok(())
    };
    match definition {
        Definition::Preset(preset) => append(preset, preset.move_ms, 0)?,
        Definition::Sequence(sequence) => {
            ensure!(
                (1..=1000).contains(&sequence.steps.len()),
                "序列步数须在 1～1000 内"
            );
            for step in &sequence.steps {
                ensure!(step.preset != action.name, "序列不能引用自身");
                let entry =
                    lookup(&step.preset).with_context(|| format!("读取预设 {}", step.preset))?;
                let Some(Definition::Preset(preset)) = entry.definition else {
                    anyhow::bail!("{} 不是动作预设", step.preset);
                };
                append(
                    &preset,
                    step.move_ms.unwrap_or(preset.move_ms),
                    step.hold_ms,
                )?;
            }
        }
    }
    validation::action(&plan, cfg)?;
    Ok(plan)
}

fn fresh(value: &Value, now: f64) -> bool {
    value["feedback_time"]
        .as_f64()
        .is_some_and(|time| (0.0..=2.0).contains(&(now - time)))
}

pub fn capture(request: &pb::PresetCapture, state: &Value, now: f64) -> Result<pb::Action> {
    let mut targets = pb::Targets {
        eye_expression: request.eye_expression,
        ..Default::default()
    };
    let arm = |key: &str, label: &str| -> Result<pb::ArmTarget> {
        let joint_rad = (0..4)
            .map(|index| {
                let feedback = &state[key][index.to_string()];
                ensure!(
                    fresh(feedback, now) && feedback["error"].as_str() == Some(""),
                    "{label}第 {} 关节没有有效位置反馈",
                    index + 1
                );
                feedback["position_rad"].as_f64().context("缺少关节角度")
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(pb::ArmTarget {
            joint_rad,
            duration_ms: request.move_ms,
        })
    };
    if request.left_arm {
        targets.left_arm = Some(arm("left_arm", "左臂")?);
    }
    if request.right_arm {
        targets.right_arm = Some(arm("right_arm", "右臂")?);
    }
    if request.neck {
        let joint = |key: &str| -> Result<f64> {
            let feedback = &state["neck"][key];
            ensure!(
                fresh(feedback, now)
                    && feedback["feedback_age_ms"]
                        .as_u64()
                        .is_some_and(|age| age <= 500),
                "头部没有有效位置反馈"
            );
            feedback["joint_position_rad"]
                .as_f64()
                .context("缺少头部角度")
        };
        targets.neck = Some(pb::NeckTarget {
            yaw_rad: Some(joint("gm6020")?),
            pitch_rad: Some(joint("dm4310")?),
        });
    }
    Ok(pb::Action {
        name: request.name.clone(),
        definition: Some(Definition::Preset(pb::ActionPreset {
            targets: Some(targets),
            move_ms: request.move_ms,
        })),
        ..Default::default()
    })
}
