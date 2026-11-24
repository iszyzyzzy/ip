use anyhow::{Context, Result, ensure};
use futures_util::{SinkExt, StreamExt};
use robot_wan::rpc::Robot;
use robotd::pb::{AudioHeader, AudioPacket, audio_packet::Payload};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use webrtc::data_channel::RTCDataChannel;

pub async fn attach(channel: Arc<RTCDataChannel>, robot: Robot) {
    let (tx, mut rx) = mpsc::channel(24);
    let (closed, mut closing) = tokio::sync::watch::channel(false);
    channel.on_close(Box::new(move || {
        let closed = closed.clone();
        Box::pin(async move {
            closed.send_replace(true);
        })
    }));
    channel.on_message(Box::new(move |msg| {
        let tx = tx.clone();
        Box::pin(async move {
            let message = if msg.is_string && msg.data.as_ref() == b"end" {
                Message::text("end")
            } else if !msg.is_string {
                Message::Binary(msg.data)
            } else {
                return;
            };
            let _ = tx.send(message).await;
        })
    }));
    let opened = channel.clone();
    channel.on_open(Box::new(move || {
        Box::pin(async move {
            tokio::spawn(async move {
                let result = tokio::select! {
                    _ = closing.changed() => return,
                    result = conversation(&opened, robot, &mut rx) => result,
                };
                if let Err(err) = result {
                    let _ = opened
                        .send_text(json!({"error":format!("{err:#}")}).to_string())
                        .await;
                }
            });
        })
    }));
}

async fn conversation(
    channel: &RTCDataChannel,
    robot: Robot,
    input: &mut mpsc::Receiver<Message>,
) -> Result<()> {
    let (mut ws, _) = connect_async("ws://127.0.0.1:8091/voice")
        .await
        .context("车载 AI 语音服务未连接")?;
    ws.send(Message::text(json!({"playback": "stream", "tts": channel.protocol()}).to_string())).await?;
    let (queue, rx) = mpsc::channel(16);
    let mut client = robot.client.clone();
    let playback = async move {
        let result = client
            .play_audio(robot.request(ReceiverStream::new(rx)))
            .await?
            .into_inner();
        ensure!(result.phase == "completed", "音响播放状态 {}", result.phase);
        Ok::<_, anyhow::Error>(())
    };
    tokio::pin!(playback);
    let mut playing = false;
    let mut queue = Some(queue);
    loop {
        tokio::select! {
            Some(message) = input.recv() => ws.send(message).await?,
            result = &mut playback, if playing => {
                result?;
                ws.send(Message::text("completed")).await?;
                playing = false;
            }
            message = ws.next() => match message.context("语音服务已断开")?? {
                Message::Text(text) => {
                    if text == "ready" { channel.send_text("ready").await?; continue }
                    let event: Value = serde_json::from_str(&text)?;
                    if let Some(rate) = event["rate"].as_u64() {
                        queue.as_ref().context("播放已结束")?.send(AudioPacket {
                            payload: Some(Payload::Header(AudioHeader { sample_rate_hz: rate.try_into()?, channels: 1 }))
                        }).await?;
                        playing = true;
                    } else if event["audio_end"] == true {
                        queue.take();
                    } else {
                        channel.send_text(text.to_string()).await?;
                        if event.get("error").is_some() || event["done"] == true { return Ok(()) }
                    }
                }
                Message::Binary(bytes) => {
                    // 发送与 gRPC 消费必须同时推进，否则长句会填满队列。
                    let send = queue.as_ref().context("播放已结束")?.send(AudioPacket { payload: Some(Payload::Pcm(bytes.to_vec())) });
                    tokio::select! { result = send => result?, result = &mut playback => { result?; anyhow::bail!("音响提前结束") } }
                }
                Message::Close(_) => anyhow::bail!("车载语音服务已关闭"),
                _ => {}
            }
        }
    }
}
