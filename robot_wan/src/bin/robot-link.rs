#[path = "robot_link/voice_channel.rs"]
mod voice_channel;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use bytes::Bytes;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use robot_wan::{
    control::Control,
    gamepad::Gamepad,
    media::{self, Talk},
    rpc::{ApiError, Robot},
    shutdown,
};
use serde_json::{Value, json};
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc, watch},
    task::JoinHandle,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use webrtc::{
    api::{
        APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{MIME_TYPE_VP8, MediaEngine},
    },
    data_channel::{RTCDataChannel, data_channel_state::RTCDataChannelState},
    ice_transport::{ice_candidate::RTCIceCandidateInit, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        RTCPeerConnection, configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::{
        RTCRtpTransceiverInit,
        rtp_codec::{RTCRtpCodecCapability, RTPCodecType},
        rtp_transceiver_direction::RTCRtpTransceiverDirection,
    },
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

#[derive(Clone, Parser)]
struct Args {
    #[arg(long)]
    signal: String,
    #[arg(long, default_value = "car-01")]
    robot_id: String,
    #[arg(long, default_value = "127.0.0.1:50051")]
    robotd: String,
    #[arg(long)]
    camera: Option<String>,
    #[arg(long, default_value = "v4l2", value_parser = ["v4l2", "lavfi"])]
    camera_format: String,
    #[arg(long, default_value = "ffmpeg")]
    ffmpeg: String,
}

type ChannelRequest = (Arc<RTCDataChannel>, Value, u64);

struct Session {
    sid: String,
    pc: RTCPeerConnection,
    control: Arc<Control>,
    talk: Arc<AsyncMutex<Talk>>,
    tasks: Vec<JoinHandle<()>>,
    microphone: Arc<Mutex<Option<JoinHandle<()>>>>,
    camera_negotiated: watch::Sender<bool>,
}

fn answer(id: &Value, result: Result<Value, ApiError>) -> String {
    match result {
        Ok(value) => json!({"type":"reply","id":id,"result":value}),
        Err(error) => json!({"type":"reply","id":id,"error":error}),
    }
    .to_string()
}

async fn commands(
    mut rx: mpsc::Receiver<ChannelRequest>,
    control: Arc<Control>,
    talk: Arc<AsyncMutex<Talk>>,
    gamepad: Arc<Gamepad>,
    emergency: bool,
) {
    while let Some((channel, msg, generation)) = rx.recv().await {
        let method = msg["method"].as_str().unwrap_or_default();
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "stop" if emergency => {
                let result = control.stop().await;
                talk.lock().await.abort().await;
                result
            }
            _ if emergency => continue,
            "talk.begin" if generation == control.generation() => {
                talk.lock().await.begin(&control.robot).await
            }
            "talk.begin" => Err(ApiError::new("FAILED_PRECONDITION", "急停已中断喊话")),
            "talk.end" => Ok(talk.lock().await.end()),
            "get_gamepad" => Ok(gamepad.status()),
            "set_gamepad" => gamepad.set_binding(&control.robot, &params).await,
            _ => control.call(method, params, generation).await,
        };
        let _ = channel.send_text(answer(&msg["id"], result)).await;
    }
}

async fn push_state(
    control: Arc<Control>,
    channel: Arc<RTCDataChannel>,
    video: watch::Receiver<&'static str>,
    talk: watch::Receiver<Value>,
) {
    let mut seq = 0;
    let mut timer = tokio::time::interval(Duration::from_millis(100));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        if channel.ready_state() == RTCDataChannelState::Closed {
            return;
        }
        if channel.ready_state() != RTCDataChannelState::Open
            || channel.buffered_amount().await != 0
        {
            continue;
        }
        let state = control.robot.call("get_state", json!({})).await;
        let (tick, epoch, dropped) = control.snapshot();
        seq += 1;
        let mut message = json!({"seq":seq,"tick":tick,"epoch":epoch,"dropped_drive":dropped,
            "video":*video.borrow(),"talk":talk.borrow().clone()});
        match state {
            Ok(state) => message["state"] = state,
            Err(error) => message["error"] = json!(ApiError::from(error)),
        }
        if channel.send_text(message.to_string()).await.is_err() {
            return;
        }
    }
}

impl Session {
    async fn new(
        sid: String,
        servers: &Value,
        robot: Robot,
        args: &Args,
        ended: mpsc::Sender<String>,
        gamepad: Arc<Gamepad>,
    ) -> Result<Self> {
        let mut m = MediaEngine::default();
        m.register_codec(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: "audio/opus".into(),
                    clock_rate: 48_000,
                    channels: 2,
                    sdp_fmtp_line: "minptime=10;useinbandfec=1".into(),
                    ..Default::default()
                },
                payload_type: 111,
                ..Default::default()
            },
            RTPCodecType::Audio,
        )?;
        m.register_codec(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_VP8.into(),
                    clock_rate: 90_000,
                    ..Default::default()
                },
                payload_type: 96,
                ..Default::default()
            },
            RTPCodecType::Video,
        )?;
        let registry = register_default_interceptors(Registry::new(), &mut m)?;
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();
        let ice_servers = servers
            .as_array()
            .context("ice_servers 必须为数组")?
            .iter()
            .map(|server| {
                let urls = server["urls"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                RTCIceServer {
                    urls,
                    username: server["username"].as_str().unwrap_or_default().into(),
                    credential: server["credential"].as_str().unwrap_or_default().into(),
                    ..Default::default()
                }
            })
            .collect();
        let pc = api
            .new_peer_connection(RTCConfiguration {
                ice_servers,
                ..Default::default()
            })
            .await?;
        let control = Control::new(robot);
        let drive_rx = control.drive.subscribe();
        let drive_control = control.clone();
        let drive_task = tokio::spawn(async move { drive_control.drive_loop(drive_rx).await });
        let talk = Talk::default();
        let pcm = talk.pcm.clone();
        let talk_state = talk.status.subscribe();
        let talk = Arc::new(AsyncMutex::new(talk));
        let (video_state, video_rx) = watch::channel("inactive");
        let (rpc, rpc_rx) = mpsc::channel(16);
        let (stop, stop_rx) = mpsc::channel(1);
        let mut tasks = vec![
            drive_task,
            tokio::spawn(commands(
                rpc_rx,
                control.clone(),
                talk.clone(),
                gamepad.clone(),
                false,
            )),
            tokio::spawn(commands(
                stop_rx,
                control.clone(),
                talk.clone(),
                gamepad,
                true,
            )),
        ];
        let microphone = Arc::new(Mutex::new(None));
        let mic = microphone.clone();
        pc.on_track(Box::new(move |track, _, _| {
            let pcm = pcm.clone();
            let mut microphone = mic.lock().unwrap();
            if track.kind() == RTPCodecType::Audio && microphone.is_none() {
                *microphone = Some(tokio::spawn(media::microphone(track, pcm)));
            }
            Box::pin(async {})
        }));
        let (connected, mut connect_rx) = watch::channel(false);
        let (camera_negotiated, mut negotiated_rx) = watch::channel(false);
        if let Some(source) = &args.camera {
            let track = Arc::new(TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_VP8.into(),
                    clock_rate: 90_000,
                    ..Default::default()
                },
                "camera".into(),
                "robot".into(),
            ));
            let transceiver = pc
                .add_transceiver_from_track(
                    track.clone(),
                    Some(RTCRtpTransceiverInit {
                        direction: RTCRtpTransceiverDirection::Sendonly,
                        send_encodings: vec![],
                    }),
                )
                .await?;
            let sender = transceiver.sender().await;
            tasks.push(tokio::spawn(async move {
                while sender.read_rtcp().await.is_ok() {}
            }));
            let (frames, rx) = watch::channel(Bytes::new());
            let source = source.clone();
            let args = args.clone();
            tasks.push(tokio::spawn(async move {
                if negotiated_rx.wait_for(|v| *v).await.is_err()
                    || connect_rx.wait_for(|v| *v).await.is_err()
                {
                    return;
                }
                video_state.send_replace("starting");
                media::camera(
                    &args.ffmpeg,
                    &args.camera_format,
                    &source,
                    frames,
                    video_state,
                )
                .await;
            }));
            tasks.push(tokio::spawn(media::send_video(track.clone(), rx)));
        }
        let accepted = Arc::new(Mutex::new(HashSet::new()));
        let close_sid = sid.clone();
        let close_tx = ended.clone();
        pc.on_peer_connection_state_change(Box::new(move |state| {
            let tx = close_tx.clone();
            let sid = close_sid.clone();
            if state == RTCPeerConnectionState::Connected {
                connected.send_replace(true);
            }
            Box::pin(async move {
                if matches!(
                    state,
                    RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed
                ) {
                    let _ = tx.try_send(sid);
                }
            })
        }));
        let ctrl = control.clone();
        let channel_sid = sid.clone();
        pc.on_data_channel(Box::new(move |channel| {
            let control = ctrl.clone();
            let accepted = accepted.clone();
            let rpc = rpc.clone();
            let stop = stop.clone();
            let video = video_rx.clone();
            let talk = talk_state.clone();
            let tx = ended.clone();
            let sid = channel_sid.clone();
            Box::pin(async move {
                let label = channel.label();
                if label == "voice.v1" {
                    voice_channel::attach(channel, control.robot.clone()).await;
                    return;
                }
                let ordered = matches!(label, "rpc.v1" | "stop.v1");
                let valid = (ordered || matches!(label, "state.v1" | "drive.v1"))
                    && channel.ordered() == ordered
                    && channel.max_retransmits() == if ordered { None } else { Some(0) }
                    && channel.max_packet_lifetime().is_none()
                    && accepted.lock().unwrap().insert(label.to_owned());
                if !valid {
                    let _ = channel.close().await;
                    return;
                }
                if label == "state.v1" {
                    tokio::spawn(push_state(control, channel, video, talk));
                    return;
                }
                channel.on_close(Box::new(move || {
                    let _ = tx.try_send(sid.clone());
                    Box::pin(async {})
                }));
                let weak = Arc::downgrade(&channel);
                channel.on_message(Box::new(move |message| {
                    let weak = weak.clone();
                    let control = control.clone();
                    let rpc = rpc.clone();
                    let stop = stop.clone();
                    Box::pin(async move {
                        if !message.is_string || message.data.len() > 60_000 {
                            return;
                        }
                        let Ok(msg) = serde_json::from_slice::<Value>(&message.data) else {
                            return;
                        };
                        if !msg.is_object() {
                            return;
                        }
                        let Some(channel) = weak.upgrade() else {
                            return;
                        };
                        match channel.label() {
                            "drive.v1" => control.offer_drive(msg),
                            "stop.v1" => {
                                let _ = stop.try_send((channel, msg, control.generation()));
                            }
                            _ => {
                                if let Err(e) = rpc.try_send((channel, msg, control.generation())) {
                                    let (channel, msg, _) = e.into_inner();
                                    let _ = channel
                                        .send_text(answer(
                                            &msg["id"],
                                            Err(ApiError::new("BUSY", "命令队列已满")),
                                        ))
                                        .await;
                                }
                            }
                        }
                    })
                }));
            })
        }));
        Ok(Self {
            sid,
            pc,
            tasks,
            control,
            talk,
            microphone,
            camera_negotiated,
        })
    }

    async fn negotiate(&self, sdp: &str) -> Result<Value> {
        ensure!(
            self.pc.remote_description().await.is_none(),
            "每个会话只支持一个 offer"
        );
        self.pc
            .set_remote_description(RTCSessionDescription::offer(sdp.to_owned())?)
            .await?;
        let mut complete = self.pc.gathering_complete_promise().await;
        self.pc
            .set_local_description(self.pc.create_answer(None).await?)
            .await?;
        let transceivers = self.pc.get_transceivers().await;
        let receives_audio = transceivers
            .iter()
            .any(|t| t.kind() == RTPCodecType::Audio && t.current_direction().has_recv());
        self.talk
            .lock()
            .await
            .track_ready
            .store(receives_audio, Ordering::Relaxed);
        self.camera_negotiated.send_replace(
            transceivers
                .iter()
                .any(|t| t.kind() == RTPCodecType::Video && t.current_direction().has_send()),
        );
        complete.recv().await;
        let response = self.pc.local_description().await.context("尚无 answer")?;
        Ok(json!({"type":"answer","sid":self.sid,"sdp":response.sdp}))
    }

    async fn close(self) {
        for task in &self.tasks {
            task.abort();
        }
        for task in self.tasks {
            let _ = task.await;
        }
        let microphone = self.microphone.lock().unwrap().take();
        if let Some(task) = microphone {
            task.abort();
            let _ = task.await;
        }
        self.control.release().await;
        self.talk.lock().await.abort().await;
        let _ = self.pc.close().await;
    }
}

async fn connection(
    args: &Args,
    token: &str,
    robot: Robot,
    gamepad: Arc<Gamepad>,
    quit: &mut watch::Receiver<bool>,
) -> Result<()> {
    let mut req = args.signal.as_str().into_client_request()?;
    req.headers_mut()
        .insert("authorization", format!("Bearer {token}").parse()?);
    let (mut ws, _) = tokio::time::timeout(Duration::from_secs(8), connect_async(req)).await??;
    ws.send(Message::Text(
        json!({"type":"robot.hello","robot_id":args.robot_id,"version":1,
        "capabilities":{"audio":true,"video":args.camera.is_some()}})
        .to_string()
        .into(),
    ))
    .await?;
    let (ended, mut closed) = mpsc::channel::<String>(8);
    let mut session: Option<Session> = None;
    let result = async {
        loop {
            let msg = tokio::select! {
                _ = quit.changed() => break,
                Some(sid) = closed.recv() => {
                    if session.as_ref().is_some_and(|s| s.sid == sid) {
                        session.take().unwrap().close().await;
                        ws.send(Message::Text(json!({"type":"session.close","sid":sid}).to_string().into())).await?;
                    }
                    continue;
                },
                frame = tokio::time::timeout(Duration::from_secs(16), ws.next()) => {
                    match frame? {
                        Some(Ok(Message::Text(text))) => serde_json::from_str::<Value>(&text)?,
                        Some(Ok(Message::Ping(_))) => { ws.flush().await?; continue; },
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(e)) => return Err(e.into()),
                        _ => continue,
                    }
                }
            };
            let sid = msg["sid"].as_str().unwrap_or_default();
            match msg["type"].as_str() {
                Some("robot.ready") => eprintln!("车辆 {} 已上线", args.robot_id),
                Some("session.opened") => {
                    if let Some(old) = session.take() { old.close().await; }
                    session = Some(Session::new(sid.into(), &msg["ice_servers"], robot.clone(),
                        args, ended.clone(), gamepad.clone()).await?);
                }
                Some("session.closed") if session.as_ref().is_some_and(|s| s.sid == sid) => {
                    session.take().unwrap().close().await;
                }
                Some(kind @ ("offer" | "ice")) if session.as_ref().is_some_and(|s| s.sid == sid) => {
                    let s = session.as_ref().unwrap();
                    if kind == "offer" {
                        let sdp = msg["sdp"].as_str().context("缺少 offer SDP")?;
                        let response = tokio::time::timeout(Duration::from_secs(20), s.negotiate(sdp)).await??;
                        ws.send(Message::Text(response.to_string().into())).await?;
                    } else {
                        let data = &msg["candidate"];
                        let candidate = if data.is_null() { RTCIceCandidateInit::default() }
                            else { serde_json::from_value(data.clone())? };
                        s.pc.add_ice_candidate(candidate).await?;
                    }
                }
                Some("error") => eprintln!("信令：{}", msg["error"]),
                _ => {}
            }
        }
        Ok(())
    }.await;
    if let Some(session) = session {
        session.close().await;
    }
    let _ = tokio::time::timeout(Duration::from_millis(500), ws.close(None)).await;
    result
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let token = std::env::var("WAN_ROBOT_TOKEN")?;
    ensure!(!token.is_empty(), "WAN_ROBOT_TOKEN 不能为空");
    let robot = Robot::new(&args.robotd, std::env::var("ROBOTD_API_TOKEN").ok())?;
    let gamepad = Arc::new(Gamepad::load());
    let pad = gamepad.clone();
    let local = robot.clone();
    let pad_task = tokio::spawn(async move { pad.run(local).await });
    let (stop, mut quit) = watch::channel(false);
    tokio::spawn(async move {
        shutdown().await;
        stop.send_replace(true);
    });
    while !*quit.borrow() {
        if let Err(error) =
            connection(&args, &token, robot.clone(), gamepad.clone(), &mut quit).await
        {
            eprintln!("信令断开：{error:#}");
        }
        if !*quit.borrow() {
            tokio::select! { _ = quit.changed() => {}, _ = tokio::time::sleep(Duration::from_secs(2)) => {} }
        }
    }
    pad_task.abort();
    Ok(())
}
