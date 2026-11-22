use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::Parser;
use hmac::{Hmac, Mac};
use robot_wan::{rpc::ApiError, shutdown};
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 8765)]
    port: u16,
    #[arg(long, default_value = "car-01")]
    robot_id: String,
    #[arg(long)]
    stun: Vec<String>,
    #[arg(long)]
    turn: Option<String>,
}

#[derive(Clone)]
struct Peer {
    id: Uuid,
    sender: mpsc::Sender<Value>,
}

struct Session {
    id: Uuid,
    robot: Peer,
    operator: Peer,
}

struct Hub {
    args: Args,
    robot_token: String,
    operator_token: String,
    turn_secret: Option<String>,
    robot: Option<(Peer, Value)>,
    session: Option<Session>,
    quit: watch::Receiver<bool>,
}

impl Hub {
    fn ice(&self, role: &str) -> Vec<Value> {
        let mut servers = Vec::new();
        if !self.args.stun.is_empty() {
            servers.push(json!({"urls":self.args.stun}));
        }
        if let (Some(turn), Some(secret)) = (&self.args.turn, &self.turn_secret) {
            let expiry = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 43200;
            let username = format!("{expiry}:{role}");
            let mut mac = Hmac::<sha1::Sha1>::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(username.as_bytes());
            servers.push(json!({"urls":[turn],"username":username,
                "credential":STANDARD.encode(mac.finalize().into_bytes())}));
        }
        servers
    }

    fn close_session(&mut self, reason: &str) {
        if let Some(session) = self.session.take() {
            let note =
                json!({"type":"session.closed","sid":session.id.to_string(),"reason":reason});
            let _ = session.robot.sender.try_send(note.clone());
            let _ = session.operator.sender.try_send(note);
        }
    }

    fn dispatch(
        &mut self,
        peer: &Peer,
        vehicle: bool,
        msg: &Value,
    ) -> Result<Option<Value>, ApiError> {
        match msg["type"].as_str().unwrap_or_default() {
            kind @ ("robot.hello" | "session.open") if vehicle == (kind == "robot.hello") => {
                if msg["robot_id"] != self.args.robot_id || msg["version"] != 1 {
                    return Err(ApiError::new(
                        "INVALID_ARGUMENT",
                        "车辆 ID 或协议版本不匹配",
                    ));
                }
                if vehicle {
                    if self.robot.as_ref().is_some_and(|(r, _)| r.id != peer.id) {
                        return Err(ApiError::new("BUSY", "车辆连接已存在"));
                    }
                    self.robot = Some((
                        peer.clone(),
                        json!({
                            "audio":msg["capabilities"]["audio"] == true,
                            "video":msg["capabilities"]["video"] == true
                        }),
                    ));
                    return Ok(Some(
                        json!({"type":"robot.ready","robot_id":self.args.robot_id,"version":1}),
                    ));
                }
                let (robot, capabilities) = self
                    .robot
                    .as_ref()
                    .ok_or_else(|| ApiError::new("ROBOT_OFFLINE", "车辆未上线"))?;
                if self.session.is_some() {
                    return Err(ApiError::new("BUSY", "车辆已有远程会话"));
                }
                let id = Uuid::new_v4();
                let opened = |role| {
                    json!({"type":"session.opened","sid":id.to_string(),
                    "robot_id":self.args.robot_id,"capabilities":capabilities,"ice_servers":self.ice(role)})
                };
                robot
                    .sender
                    .try_send(opened("robot"))
                    .map_err(|_| ApiError::new("ROBOT_OFFLINE", "车辆连接不可用"))?;
                let response = opened("operator");
                self.session = Some(Session {
                    id,
                    robot: robot.clone(),
                    operator: peer.clone(),
                });
                Ok(Some(response))
            }
            "robots.list" if !vehicle => Ok(Some(json!({"type":"robots.list","robots":[{
                "robot_id":self.args.robot_id, "online":self.robot.is_some(), "busy":self.session.is_some(),
                "capabilities":self.robot.as_ref().map(|(_, v)| v.clone()).unwrap_or_else(|| json!({}))
            }]}))),
            kind @ ("offer" | "answer" | "ice" | "session.close") => {
                let session = self
                    .session
                    .as_ref()
                    .filter(|s| {
                        msg["sid"] == s.id.to_string()
                            && (peer.id == s.robot.id || peer.id == s.operator.id)
                    })
                    .ok_or_else(|| ApiError::new("NOT_FOUND", "会话不存在"))?;
                if kind == "session.close" {
                    self.close_session("closed");
                } else {
                    if (kind == "offer" && vehicle) || (kind == "answer" && !vehicle) {
                        return Err(ApiError::new("INVALID_ARGUMENT", "SDP 发送方向错误"));
                    }
                    let field = if kind == "ice" { "candidate" } else { "sdp" };
                    let target = if vehicle {
                        &session.operator
                    } else {
                        &session.robot
                    };
                    let mut note = json!({"type":kind,"sid":msg["sid"]});
                    note[field] = msg[field].clone();
                    target
                        .sender
                        .try_send(note)
                        .map_err(|_| ApiError::new("UNAVAILABLE", "对端连接不可用"))?;
                }
                Ok(None)
            }
            _ => Err(ApiError::new("INVALID_ARGUMENT", "未知信令类型")),
        }
    }
}

type Shared = Arc<Mutex<Hub>>;

async fn upgrade(State(hub): State<Shared>, headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let role = {
        let hub = hub.lock().unwrap();
        auth.and_then(|v| {
            if v == hub.robot_token {
                Some(true)
            } else if v == hub.operator_token {
                Some(false)
            } else {
                None
            }
        })
    };
    match role {
        Some(vehicle) => ws
            .max_message_size(128 * 1024)
            .on_upgrade(move |socket| handle(hub, socket, vehicle)),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn handle(hub: Shared, mut socket: WebSocket, vehicle: bool) {
    let mut quit = hub.lock().unwrap().quit.clone();
    let (sender, mut receive) = mpsc::channel(64);
    let peer = Peer {
        id: Uuid::new_v4(),
        sender,
    };
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(5));
    let mut last_rx = tokio::time::Instant::now();
    while !*quit.borrow() {
        let outgoing = tokio::select! {
            _ = quit.changed() => break,
            _ = ping.tick() => {
                if last_rx.elapsed().as_secs() >= 15 { break; }
                Some(Message::Ping(Default::default()))
            }
            note = receive.recv() => note.map(|v| Message::Text(v.to_string().into())),
            frame = socket.recv() => {
                let Some(Ok(frame)) = frame else { break };
                last_rx = tokio::time::Instant::now();
                let text = match frame { Message::Text(text) => text, Message::Close(_) => break, _ => continue };
                let parsed = serde_json::from_str::<Value>(&text);
                let (id, result) = match parsed {
                    Ok(msg) if msg.is_object() => (
                        msg["id"].clone(), hub.lock().unwrap().dispatch(&peer, vehicle, &msg)),
                    _ => (Value::Null, Err(ApiError::new("INVALID_ARGUMENT", "JSON 必须是对象"))),
                };
                let mut reply = match result {
                    Ok(Some(reply)) => reply,
                    Ok(None) => continue,
                    Err(error) => json!({"type":"error","error":error}),
                };
                reply["id"] = id;
                Some(Message::Text(reply.to_string().into()))
            }
        };
        if let Some(message) = outgoing
            && !matches!(
                tokio::time::timeout(std::time::Duration::from_secs(5), socket.send(message)).await,
                Ok(Ok(()))
            )
        {
            break;
        }
    }
    let mut hub = hub.lock().unwrap();
    if hub.robot.as_ref().is_some_and(|(r, _)| r.id == peer.id) {
        hub.robot = None;
    }
    if hub
        .session
        .as_ref()
        .is_some_and(|s| s.robot.id == peer.id || s.operator.id == peer.id)
    {
        hub.close_session("peer_disconnected");
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let robot_token = std::env::var("WAN_ROBOT_TOKEN")?;
    let operator_token = std::env::var("WAN_OPERATOR_TOKEN")?;
    anyhow::ensure!(
        !robot_token.is_empty() && !operator_token.is_empty() && robot_token != operator_token,
        "车辆和操作员须使用不同的非空凭据"
    );
    let turn_secret = std::env::var("WAN_TURN_SECRET")
        .ok()
        .filter(|v| !v.is_empty());
    anyhow::ensure!(
        args.turn.is_some() == turn_secret.is_some(),
        "TURN 地址与共享密钥须一起配置"
    );
    let listener = tokio::net::TcpListener::bind((args.host.as_str(), args.port)).await?;
    let (stop, quit) = watch::channel(false);
    let hub = Hub {
        args,
        robot_token,
        operator_token,
        turn_secret,
        robot: None,
        session: None,
        quit,
    };
    let app = Router::new()
        .route("/healthz", get(|| async { Json(json!({"ok":true})) }))
        .route("/v1/signal", get(upgrade))
        .with_state(Arc::new(Mutex::new(hub)));
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown().await;
            stop.send_replace(true);
        })
        .await?;
    Ok(())
}
