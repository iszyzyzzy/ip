use std::{process::Stdio, sync::Arc, time::Duration};

use anyhow::{Result, ensure};
use bytes::Bytes;
use opus_decoder::OpusDecoder;
use robotd::pb::{AudioHeader, AudioPacket, audio_packet::Payload};
use serde_json::{Value, json};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use webrtc::{
    media::Sample,
    track::{
        track_local::track_local_static_sample::TrackLocalStaticSample, track_remote::TrackRemote,
    },
};

use crate::rpc::{ApiError, Robot};

pub async fn camera(
    ffmpeg: &str,
    format: &str,
    source: &str,
    frames: watch::Sender<Bytes>,
    status: watch::Sender<&'static str>,
) {
    loop {
        if let Err(error) = capture(ffmpeg, format, source, &frames, &status).await
            && status.send_replace("offline") != "offline"
        {
            eprintln!("摄像头采集失败：{error}");
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn capture(
    ffmpeg: &str,
    format: &str,
    source: &str,
    frames: &watch::Sender<Bytes>,
    status: &watch::Sender<&str>,
) -> Result<()> {
    let mut command = Command::new(ffmpeg);
    command.args(["-nostdin", "-loglevel", "error", "-f", format]);
    if format == "v4l2" {
        command.args(["-framerate", "15", "-video_size", "640x480"]);
    } else {
        command.arg("-re");
    }
    let mut child = command
        .args([
            "-i",
            source,
            "-an",
            "-vf",
            "scale=640:480",
            "-r",
            "15",
            "-c:v",
            "libvpx",
            "-deadline",
            "realtime",
            "-cpu-used",
            "6",
            "-lag-in-frames",
            "0",
            "-g",
            "15",
            "-b:v",
            "900k",
            "-pix_fmt",
            "yuv420p",
            "-f",
            "ivf",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut header = [0; 32];
    tokio::time::timeout(Duration::from_secs(3), stdout.read_exact(&mut header)).await??;
    ensure!(
        &header[..4] == b"DKIF" && &header[8..12] == b"VP80",
        "FFmpeg 未输出 VP8"
    );
    loop {
        let frame = async {
            let mut header = [0; 12];
            stdout.read_exact(&mut header).await?;
            let size = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
            ensure!((1..=1_048_576).contains(&size), "IVF 帧长度无效");
            let mut payload = vec![0; size];
            stdout.read_exact(&mut payload).await?;
            Ok::<_, anyhow::Error>(Bytes::from(payload))
        };
        frames.send_replace(tokio::time::timeout(Duration::from_secs(3), frame).await??);
        status.send_replace("live");
    }
}

pub async fn send_video(track: Arc<TrackLocalStaticSample>, mut frames: watch::Receiver<Bytes>) {
    // FFmpeg 周期性生成关键帧，丢帧后接收端可从下一关键帧恢复。
    while frames.changed().await.is_ok() {
        let data = frames.borrow_and_update().clone();
        let sample = Sample {
            data,
            duration: Duration::from_micros(66_667),
            ..Default::default()
        };
        if track.write_sample(&sample).await.is_err() {
            return;
        }
    }
}

type Pcm = mpsc::Sender<AudioPacket>;

pub struct Talk {
    pub pcm: watch::Sender<Option<Pcm>>,
    pub status: watch::Sender<Value>,
    pub track_ready: Arc<std::sync::atomic::AtomicBool>,
    task: Option<JoinHandle<()>>,
}

impl Default for Talk {
    fn default() -> Self {
        let (pcm, _) = watch::channel(None);
        let (status, _) = watch::channel(json!({"phase":"idle"}));
        Self {
            pcm,
            status,
            track_ready: Default::default(),
            task: None,
        }
    }
}

impl Talk {
    pub async fn begin(&mut self, robot: &Robot) -> Result<Value, ApiError> {
        if !self.track_ready.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ApiError::new("AUDIO_OFFLINE", "会话没有 Opus 麦克风音轨"));
        }
        if self.task.as_ref().is_some_and(|task| !task.is_finished()) {
            return Err(ApiError::new("AUDIO_BUSY", "上一段喊话仍在播放"));
        }
        let status = robot.call("get_audio_state", json!({})).await?;
        match status["phase"].as_str() {
            Some("disabled") => return Err(ApiError::new("AUDIO_OFFLINE", "robotd 尚未启用音频")),
            Some("buffering" | "playing" | "rebuffering" | "draining" | "stopping") => {
                return Err(ApiError::new("AUDIO_BUSY", "车载播放器正忙"));
            }
            _ => {}
        }
        let (queue, rx) = mpsc::channel(3);
        queue
            .try_send(AudioPacket {
                payload: Some(Payload::Header(AudioHeader {
                    sample_rate_hz: 48_000,
                    channels: 1,
                })),
            })
            .unwrap();
        let status = json!({"phase":"accepted"});
        self.status.send_replace(status.clone());
        self.pcm.send_replace(Some(queue));
        let robot = robot.clone();
        let phase = self.status.clone();
        let pcm = self.pcm.clone();
        self.task = Some(tokio::spawn(async move {
            let mut client = robot.client.clone();
            let packets = ReceiverStream::new(rx)
                .timeout(Duration::from_secs(2))
                .take_while(|packet| packet.is_ok())
                .map(Result::unwrap);
            let req = robot.request(packets);
            let status = match client.play_audio(req).await {
                Ok(response) => json!({"phase":response.into_inner().phase}),
                Err(e) => {
                    let mut error = ApiError::from(e);
                    if error.code == "RESOURCE_EXHAUSTED" {
                        error.code = "AUDIO_BUSY".into();
                    }
                    json!({"phase":"failed","code":error.code,"message":error.message})
                }
            };
            pcm.send_replace(None);
            phase.send_replace(status);
        }));
        Ok(status)
    }

    pub fn end(&self) -> Value {
        self.pcm.send_replace(None);
        json!({"phase":self.status.borrow()["phase"]})
    }

    pub async fn abort(&mut self) {
        self.pcm.send_replace(None);
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
        self.status.send_replace(json!({"phase":"idle"}));
    }
}

pub async fn microphone(track: Arc<TrackRemote>, pcm: watch::Sender<Option<Pcm>>) {
    let Ok(mut decoder) = OpusDecoder::new(48_000, 1) else {
        return;
    };
    let mut decoded = [0_i16; 5760];
    loop {
        match tokio::time::timeout(Duration::from_secs(2), track.read_rtp()).await {
            Ok(Ok((packet, _))) => {
                let queue = pcm.borrow().clone();
                if let Some(queue) = queue
                    && let Ok(count) = decoder.decode(&packet.payload, &mut decoded, false)
                {
                    let data = decoded[..count]
                        .iter()
                        .flat_map(|sample| sample.to_le_bytes())
                        .collect();
                    let _ = queue.try_send(AudioPacket {
                        payload: Some(Payload::Pcm(data)),
                    });
                }
            }
            Ok(Err(_)) => break,
            Err(_) => {
                pcm.send_replace(None);
            }
        }
    }
    pcm.send_replace(None);
}
