//! cargo run --example controller -- [socket-path] [state|demo|blink|idle|clean|show|reload]
use eyes::control::{connect, pb};
use std::{path::PathBuf, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("oculus-eyes.sock"));
    let action = args.next().unwrap_or_else(|| "demo".into());
    let mut client = connect(path).await?;
    match action.as_str() {
        "reload" => {
            println!(
                "{:?}",
                client.reload_assets(pb::Empty {}).await?.into_inner()
            );
        }
        "state" => println!(
            "{}",
            serde_json::to_string_pretty(&client.get_state(pb::Empty {}).await?.into_inner())?
        ),
        "configure" => {
            let file = args.next().ok_or("configure requires a JSON file")?;
            let config: pb::Config = serde_json::from_str(&std::fs::read_to_string(file)?)?;
            println!("{:?}", client.configure(config).await?.into_inner());
        }
        "sim" => {
            let x: f32 = args.next().unwrap_or_else(|| "0".into()).parse()?;
            let y: f32 = args.next().unwrap_or_else(|| "0".into()).parse()?;
            client
                .configure(pb::Config {
                    mode: Some(pb::Mode::Simulator as i32),
                    simulator: Some(pb::SimulatorConfig {
                        target_x: Some(x),
                        target_y: Some(y),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .await?;
        }
        "jelly" => {
            let enabled = parse_toggle(args.next())?;
            client
                .configure(pb::Config {
                    jelly: Some(pb::JellyConfig {
                        enabled: Some(enabled),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .await?;
        }
        "layer" => {
            let name = args.next().ok_or("layer requires a layer name")?;
            let enabled = parse_toggle(args.next())?;
            let value = serde_json::json!({ name: enabled });
            let layers: pb::LayerConfig = serde_json::from_value(value)?;
            client
                .configure(pb::Config {
                    layers: Some(layers),
                    ..Default::default()
                })
                .await?;
        }
        "wink" => {
            let side = args.next().ok_or("wink requires left or right")?;
            if side != "left" && side != "right" {
                return Err("wink requires left or right".into());
            }
            client
                .trigger_blink(pb::BlinkRequest {
                    left: Some(side == "left"),
                    right: Some(side == "right"),
                    duration: None,
                })
                .await?;
        }
        "save" => {
            client.save_settings(pb::Empty {}).await?;
        }
        "connect" => {
            client.connect_tracking(pb::Empty {}).await?;
        }
        "blink" => {
            println!("{:?}", client.blink(pb::Empty {}).await?.into_inner());
        }

        "idle" => {
            client
                .configure(pb::Config {
                    mode: Some(pb::Mode::Idle as i32),
                    ..Default::default()
                })
                .await?;
        }
        "clean" | "show" => {
            client
                .configure(pb::Config {
                    show_controls: Some(action == "show"),
                    ..Default::default()
                })
                .await?;
        }
        "demo" => {
            client
                .configure(pb::Config {
                    iris: Some(pb::Color {
                        r: 0.6,
                        g: 0.3,
                        b: 0.95,
                    }),
                    ..Default::default()
                })
                .await?;
            let (tx, rx) = tokio::sync::mpsc::channel(2);
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_millis(16));
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                for frame in 0..600 {
                    tick.tick().await;
                    let t = frame as f32 / 60.;
                    let eye = pb::Eye {
                        gaze_x: (t * 1.2).sin() * 0.8,
                        gaze_y: (t * 0.7).cos() * 0.4,
                        openness: 1.,
                        brow: (t * 0.5).sin() * 0.5,
                    };
                    let left = pb::Eye {
                        openness: if (t % 3.) < 0.18 { 0. } else { 1. },
                        ..eye
                    };
                    if tx
                        .send(pb::Pose {
                            left: Some(left),
                            right: Some(eye),
                            roll: (t * 0.4).sin() * 0.12,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });
            let ack = client
                .stream_poses(tokio_stream::wrappers::ReceiverStream::new(rx))
                .await?
                .into_inner();
            println!("600 poses applied; revision {}", ack.revision);
            client
                .configure(pb::Config {
                    mode: Some(pb::Mode::Idle as i32),
                    ..Default::default()
                })
                .await?;
        }
        _ => return Err(format!("Unknown action {action}").into()),
    }
    Ok(())
}

fn parse_toggle(value: Option<String>) -> Result<bool, Box<dyn std::error::Error>> {
    match value.as_deref() {
        Some("on") => Ok(true),
        Some("off") => Ok(false),
        _ => Err("expected on or off".into()),
    }
}
