mod signal;

use std::path::PathBuf;

use serde_json::Value;
use signal::SignalManager;
use tauri::{AppHandle, Manager};

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录：{e}"))?;
    Ok(dir.join("client-config.json"))
}

#[tauri::command]
fn load_client_config(app: AppHandle) -> Result<Value, String> {
    let path = config_path(&app)?;
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("解析本地配置失败：{e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(format!("读取本地配置失败：{e}")),
    }
}

#[tauri::command]
fn save_client_config(app: AppHandle, config: Value) -> Result<(), String> {
    let path = config_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let tmp = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(&config).map_err(|e| format!("序列化配置失败：{e}"))?;
    std::fs::write(&tmp, data).map_err(|e| format!("写入临时配置失败：{e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("保存配置失败：{e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 连接任务使用进程级默认 provider，必须在 Tauri 启动异步任务前选定。
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("无法初始化 TLS 加密提供者");

    let manager = SignalManager::default();
    let on_exit = manager.clone();

    tauri::Builder::default()
        .manage(manager)
        .invoke_handler(tauri::generate_handler![
            signal::signal_connect,
            signal::signal_send,
            signal::signal_disconnect,
            load_client_config,
            save_client_config,
        ])
        .on_window_event(move |_window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let m = on_exit.clone();
                tauri::async_runtime::spawn(async move {
                    m.disconnect().await;
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 客户端失败");
}
