//! 对无窗口进程验证 UDP 面捕和 gRPC；可用于 DRM、离屏 GPU 或纯控制后端。
use eyes::control::{connect, pb};
use std::{net::UdpSocket, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::args().nth(1).expect("socket path");
    let mut client = connect(socket).await?;
    let state = client.get_state(pb::Empty {}).await?.into_inner();
    assert!(["drm", "offscreen", "control_only"].contains(&state.output_mode.as_str()));
    client
        .configure(pb::Config {
            mode: Some(pb::Mode::Simulator as i32),
            auto_blink: Some(false),
            simulator: Some(pb::SimulatorConfig {
                target_x: Some(0.6),
                target_y: Some(-0.3),
                eye_motion: Some(false),
                ..Default::default()
            }),
            layers: Some(pb::LayerConfig {
                highlights: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let state = client.get_state(pb::Empty {}).await?.into_inner();
    assert!((state.rendered_pose.unwrap().left.unwrap().gaze_x - 0.6).abs() < 0.02);
    assert_eq!(
        state.config.unwrap().layers.unwrap().highlights,
        Some(false)
    );
    let port_probe = UdpSocket::bind("127.0.0.1:0")?;
    let port = port_probe.local_addr()?.port();
    drop(port_probe);
    client
        .configure(pb::Config {
            mode: Some(pb::Mode::Ifacialmocap as i32),
            tracking_port: Some(port.into()),
            phone_ip: Some("127.0.0.1".into()),
            mirror_tracking: Some(false),
            ..Default::default()
        })
        .await?;
    let phone = UdpSocket::bind("127.0.0.1:49983")?;
    phone.set_read_timeout(Some(Duration::from_secs(2)))?;
    client.connect_tracking(pb::Empty {}).await?;
    let mut packet = [0; 256];
    let (len, peer) = phone.recv_from(&mut packet)?;
    assert_eq!(peer.port(), port);
    assert!(std::str::from_utf8(&packet[..len])?.contains("sendDataVersion=v2"));
    let udp = UdpSocket::bind("127.0.0.1:0")?;
    for _ in 0..20 {
        udp.send_to(
            b"eyeBlink_L&100|eyeBlink_R&0|eyeLookOut_L&75|eyeLookUp_L&25|eyeLookOut_R&30|",
            ("127.0.0.1", port),
        )?;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let tracked = client.get_state(pb::Empty {}).await?.into_inner();
    assert!(tracked.tracking_live);
    assert!(tracked.tracking_packets >= 20);
    let pose = tracked.rendered_pose.unwrap();
    let left = pose.left.unwrap();
    let right = pose.right.unwrap();
    assert!(left.openness < 0.01);
    assert!(right.openness > 0.99);
    assert!((left.gaze_x - 0.75).abs() < 0.03);
    assert!((left.gaze_y + 0.25).abs() < 0.03);
    assert!((right.gaze_x + 0.3).abs() < 0.03);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        !client
            .get_state(pb::Empty {})
            .await?
            .into_inner()
            .tracking_live
    );
    // 重绑失败必须保留整个旧配置，不能让下一帧失去仍可用的面捕入口。
    let occupied = UdpSocket::bind("0.0.0.0:0")?;
    let before = client.get_state(pb::Empty {}).await?.into_inner();
    assert_eq!(
        client
            .configure(pb::Config {
                tracking_port: Some(occupied.local_addr()?.port().into()),
                scale: Some(1.3),
                ..Default::default()
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::FailedPrecondition
    );
    let after = client.get_state(pb::Empty {}).await?.into_inner();
    assert_eq!(before.revision, after.revision);
    let before_config = before.config.unwrap();
    let after_config = after.config.unwrap();
    assert_eq!(before_config.scale, after_config.scale);
    assert_eq!(before_config.tracking_port, after_config.tracking_port);
    let eye = |openness| {
        Some(pb::Eye {
            gaze_x: 0.,
            gaze_y: 0.,
            openness,
            brow: 0.,
        })
    };
    client
        .set_pose(pb::Pose {
            left: eye(0.),
            right: eye(1.),
            roll: 0.,
        })
        .await?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    let pose = client
        .get_state(pb::Empty {})
        .await?
        .into_inner()
        .rendered_pose
        .unwrap();
    assert!(pose.left.unwrap().openness < 0.01);
    assert!(pose.right.unwrap().openness > 0.99);
    client.reload_assets(pb::Empty {}).await?;
    client
        .configure(pb::Config {
            mode: Some(pb::Mode::Ifacialmocap as i32),
            ..Default::default()
        })
        .await?;
    client.save_settings(pb::Empty {}).await?;
    println!(
        "通过：无屏 gRPC、模拟坐标、UDP 面捕姿态和超时、启动握手、端口回滚、独立眼姿态、素材重载和面捕模式保存"
    );
    Ok(())
}
