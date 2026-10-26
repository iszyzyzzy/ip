//! Run against an app launched with a disposable --settings path.
use eyes::control::{connect, pb};
use std::time::Duration;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::args().nth(1).expect("socket path");
    let mut c = connect(socket).await?;
    c.configure(pb::Config {
        mode: Some(pb::Mode::Simulator as i32),
        auto_blink: Some(false),
        simulator: Some(pb::SimulatorConfig {
            target_x: Some(0.75),
            target_y: Some(-0.5),
            eye_motion: Some(false),
            ..Default::default()
        }),
        jelly: Some(pb::JellyConfig {
            enabled: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(650)).await;
    let state = c.get_state(pb::Empty {}).await?.into_inner();
    let pose = state.rendered_pose.unwrap();
    assert!((pose.left.unwrap().gaze_x - 0.75).abs() < 0.015);
    assert!((pose.right.unwrap().gaze_y + 0.5).abs() < 0.015);
    c.configure(pb::Config {
        simulator: Some(pb::SimulatorConfig {
            target_x: Some(-0.6),
            ..Default::default()
        }),
        layers: Some(pb::LayerConfig {
            iris: Some(false),
            ..Default::default()
        }),
        jelly: Some(pb::JellyConfig {
            enabled: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await?;
    let mut displaced = false;
    for _ in 0..12 {
        tokio::time::sleep(Duration::from_millis(25)).await;
        let s = c.get_state(pb::Empty {}).await?.into_inner();
        displaced |= s.jelly_left.iter().any(|v| v.abs() > 0.001);
        let config = s.config.unwrap();
        assert_eq!(config.simulator.unwrap().target_y, Some(-0.5));
        assert_eq!(config.layers.unwrap().pupil, Some(true));
        assert_eq!(config.layers.unwrap().iris, Some(false));
    }
    assert!(displaced, "jelly must cause real deformation");
    c.configure(pb::Config {
        jelly: Some(pb::JellyConfig {
            enabled: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await?;
    assert_eq!(
        c.get_state(pb::Empty {}).await?.into_inner().jelly_left,
        vec![0.; 4]
    );
    c.trigger_blink(pb::BlinkRequest {
        left: Some(true),
        right: Some(false),
        duration: Some(0.6),
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(260)).await;
    let p = c
        .get_state(pb::Empty {})
        .await?
        .into_inner()
        .rendered_pose
        .unwrap();
    assert!(p.left.unwrap().openness < 0.15);
    assert!(p.right.unwrap().openness > 0.99);
    let invalid = pb::Config {
        simulator: Some(pb::SimulatorConfig {
            target_x: Some(f32::NAN),
            ..Default::default()
        }),
        layers: Some(pb::LayerConfig {
            pupil: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        c.configure(invalid).await.unwrap_err().code(),
        tonic::Code::InvalidArgument
    );
    assert_eq!(
        c.get_state(pb::Empty {})
            .await?
            .into_inner()
            .config
            .unwrap()
            .layers
            .unwrap()
            .pupil,
        Some(true)
    );
    // A failed UDP rebind must not apply any of the same patch's other fields.
    let occupied = std::net::UdpSocket::bind("0.0.0.0:0")?;
    assert_eq!(
        c.configure(pb::Config {
            tracking_port: Some(occupied.local_addr()?.port() as u32),
            simulator: Some(pb::SimulatorConfig {
                target_x: Some(0.),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .unwrap_err()
        .code(),
        tonic::Code::FailedPrecondition
    );
    assert_eq!(
        c.get_state(pb::Empty {})
            .await?
            .into_inner()
            .config
            .unwrap()
            .simulator
            .unwrap()
            .target_x,
        Some(-0.6)
    );
    c.configure(pb::Config {
        simulator: Some(pb::SimulatorConfig {
            eye_motion: Some(true),
            motion_amplitude: Some(0.15),
            motion_speed: Some(3.),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await?;
    let a = c
        .get_state(pb::Empty {})
        .await?
        .into_inner()
        .rendered_pose
        .unwrap()
        .left
        .unwrap()
        .gaze_x;
    tokio::time::sleep(Duration::from_millis(450)).await;
    let b = c
        .get_state(pb::Empty {})
        .await?
        .into_inner()
        .rendered_pose
        .unwrap()
        .left
        .unwrap()
        .gaze_x;
    assert!((a - b).abs() > 0.003, "simulated eye movements");
    c.configure(pb::Config {
        auto_blink: Some(true),
        blink_interval: Some(0.5),
        blink_duration: Some(0.35),
        blink_variation: Some(0.),
        ..Default::default()
    })
    .await?;
    let mut closed = false;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(30)).await;
        closed |= c
            .get_state(pb::Empty {})
            .await?
            .into_inner()
            .rendered_pose
            .unwrap()
            .right
            .unwrap()
            .openness
            < 0.2;
    }
    assert!(closed, "automatic blinking");
    // Full runtime settings readback must be accepted as a patch.
    let config = c
        .get_state(pb::Empty {})
        .await?
        .into_inner()
        .config
        .unwrap();
    c.configure(config).await?;
    c.save_settings(pb::Empty {}).await?;
    println!(
        "PASS: normalized target, nested patches, jelly deformation/disable, independent layers, wink, validation, atomic UDP rebind, saccades, auto blink, full config roundtrip and SaveSettings"
    );
    Ok(())
}
