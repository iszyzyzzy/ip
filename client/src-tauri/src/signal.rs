use std::{sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{Mutex, mpsc, watch};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error as WsError, Message, client::IntoClientRequest},
};

struct ActiveSignal {
    id: u64,
    tx: mpsc::Sender<String>,
    close: watch::Sender<bool>,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    active: Option<ActiveSignal>,
}

/// 管理当前活动的公网信令 WebSocket 连接，为前端绕开浏览器不能自定义 Authorization 头限制。
#[derive(Clone, Default)]
pub struct SignalManager {
    inner: Arc<Mutex<Inner>>,
}

impl SignalManager {
    pub async fn disconnect(&self) {
        let old = {
            let mut guard = self.inner.lock().await;
            guard.active.take()
        };
        if let Some(conn) = old {
            let _ = conn.close.send(true);
        }
    }
}

fn format_connect_error(err: WsError) -> String {
    if let WsError::Http(resp) = &err
        && resp.status().as_u16() == 401
    {
        return "认证失败（401），请检查操作员 Token 是否正确".into();
    }
    format!("连接信令服务失败：{err}")
}

#[tauri::command]
pub async fn signal_connect(
    app: AppHandle,
    manager: State<'_, SignalManager>,
    url: String,
    token: String,
) -> Result<(), String> {
    let url = url.trim().to_string();
    let token = token.trim().to_string();
    if url.is_empty() {
        return Err("信令地址不能为空".into());
    }
    if token.is_empty() {
        return Err("操作员 Token 不能为空".into());
    }

    // 先断开既有连接，避免并发会话残留在服务端。
    manager.disconnect().await;

    let mut req = url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("信令 URL 格式无效：{e}"))?;
    let auth_header = format!("Bearer {token}")
        .parse()
        .map_err(|e| format!("Token 格式无效：{e}"))?;
    req.headers_mut().insert("authorization", auth_header);

    let (mut ws, _) = tokio::time::timeout(Duration::from_secs(15), connect_async(req))
        .await
        .map_err(|_| "连接信令服务超时（15 秒）".to_string())?
        .map_err(format_connect_error)?;

    let (tx, mut rx) = mpsc::channel::<String>(64);
    let (close_tx, mut close_rx) = watch::channel(false);

    let conn_id = {
        let mut guard = manager.inner.lock().await;
        guard.next_id += 1;
        let id = guard.next_id;
        guard.active = Some(ActiveSignal {
            id,
            tx,
            close: close_tx,
        });
        id
    };

    let _ = app.emit("signal:status", json!({ "state": "connected", "reason": "" }));

    let shared = manager.inner().clone();
    tokio::spawn(async move {
        let mut ping = tokio::time::interval(Duration::from_secs(15));
        // 首次 tick 立即触发，跳过它以免刚建链就额外发 Ping。
        ping.tick().await;
        let mut reason = String::from("信令连接已关闭");

        loop {
            tokio::select! {
                _ = close_rx.changed() => {
                    reason = "主动断开".into();
                    let _ = tokio::time::timeout(Duration::from_millis(500), ws.close(None)).await;
                    break;
                }
                _ = ping.tick() => {
                    if ws.send(Message::Ping(Default::default())).await.is_err() {
                        reason = "信令心跳发送失败".into();
                        break;
                    }
                }
                outgoing = rx.recv() => {
                    let Some(text) = outgoing else { break };
                    if let Err(e) = ws.send(Message::Text(text.into())).await {
                        reason = format!("发送信令失败：{e}");
                        break;
                    }
                }
                incoming = tokio::time::timeout(Duration::from_secs(45), ws.next()) => {
                    match incoming {
                        Ok(Some(Ok(Message::Text(text)))) => {
                            let _ = app.emit("signal:message", text.to_string());
                        }
                        Ok(Some(Ok(Message::Ping(_)))) => {
                            if ws.flush().await.is_err() {
                                reason = "响应信令 Ping 失败".into();
                                break;
                            }
                        }
                        Ok(Some(Ok(Message::Close(frame)))) => {
                            reason = frame
                                .map(|f| f.reason.to_string())
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| "服务端关闭连接".into());
                            break;
                        }
                        Ok(Some(Ok(_))) => {}
                        Ok(Some(Err(e))) => {
                            reason = format!("信令读取错误：{e}");
                            break;
                        }
                        Ok(None) => {
                            reason = "信令流结束".into();
                            break;
                        }
                        Err(_) => {
                            reason = "信令接收超时".into();
                            break;
                        }
                    }
                }
            }
        }

        let mut is_current = false;
        {
            let mut guard = shared.inner.lock().await;
            if guard.active.as_ref().is_some_and(|a| a.id == conn_id) {
                guard.active = None;
                is_current = true;
            }
        }
        if is_current {
            let _ = app.emit("signal:status", json!({ "state": "closed", "reason": reason }));
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn signal_send(
    manager: State<'_, SignalManager>,
    payload: String,
) -> Result<(), String> {
    let tx = {
        let guard = manager.inner.lock().await;
        guard
            .active
            .as_ref()
            .map(|a| a.tx.clone())
            .ok_or_else(|| "信令未连接".to_string())?
    };
    tx.send(payload)
        .await
        .map_err(|_| "信令发送通道已关闭".to_string())
}

#[tauri::command]
pub async fn signal_disconnect(manager: State<'_, SignalManager>) -> Result<(), String> {
    manager.disconnect().await;
    Ok(())
}
