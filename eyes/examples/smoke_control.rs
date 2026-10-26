//! Integration test against a running window: cargo run --example smoke_control -- SOCKET
use eyes::control::{connect, pb};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("socket path");
    let mut client = connect(path).await?;
    let initial = client.get_state(pb::Empty {}).await?.into_inner();
    let initial_config = initial.config.expect("initialized UI state");
    let ack = client
        .configure(pb::Config {
            expression: Some(0.7),
            ..Default::default()
        })
        .await?
        .into_inner();
    let state = client.get_state(pb::Empty {}).await?.into_inner();
    assert!(state.revision >= ack.revision);
    assert_eq!(state.config.as_ref().unwrap().expression, Some(0.7));
    assert_eq!(state.config.as_ref().unwrap().iris, initial_config.iris); // partial update preserves color
    let eye = pb::Eye {
        gaze_x: 0.7,
        gaze_y: -0.3,
        openness: 1.,
        brow: 0.2,
    };
    let pose = pb::Pose {
        left: Some(pb::Eye {
            openness: 0.,
            ..eye
        }),
        right: Some(eye),
        roll: 0.1,
    };
    client.set_pose(pose).await?;
    tokio::time::sleep(Duration::from_millis(650)).await;
    let state = client.get_state(pb::Empty {}).await?.into_inner();
    assert_eq!(state.config.unwrap().mode, Some(pb::Mode::External as i32));
    let rendered = state.rendered_pose.unwrap();
    assert!(rendered.left.unwrap().openness < 0.01);
    assert!((rendered.right.unwrap().gaze_x - 0.7).abs() < 0.02);
    assert!((rendered.roll - 0.1).abs() < 0.01);
    assert_eq!(
        client
            .configure(pb::Config {
                scale: Some(f32::INFINITY),
                ..Default::default()
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    let frames = vec![pose; 10];
    client.stream_poses(tokio_stream::iter(frames)).await?;
    client.blink(pb::Empty {}).await?;
    client
        .configure(pb::Config {
            mode: Some(pb::Mode::Ifacialmocap as i32),
            ..Default::default()
        })
        .await?;
    let udp = std::net::UdpSocket::bind("127.0.0.1:0")?;
    for _ in 0..10 {
        udp.send_to(
            b"eyeBlink_L&100|eyeBlink_R&0|eyeLookOut_L&75|eyeLookIn_R&75|",
            "127.0.0.1:49983",
        )?;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let tracked = client.get_state(pb::Empty {}).await?.into_inner();
    assert!(tracked.tracking_live);
    assert!(tracked.tracking_packets >= 10);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        !client
            .get_state(pb::Empty {})
            .await?
            .into_inner()
            .tracking_live
    );
    client.configure(initial_config).await?;
    println!(
        "PASS: config patch, applied acknowledgments, left wink, gaze smoothing, roll, invalid values, streaming, blink, UDP reception and timeout fallback"
    );
    Ok(())
}
