use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::sync::watch;
use uuid::Uuid;

use crate::rpc::{ApiError, Robot};

#[derive(Clone)]
struct Lease {
    token: String,
    epoch: String,
    deadline: Instant,
    seq: u64,
}

#[derive(Default)]
struct State {
    lease: Option<Lease>,
    ticks: VecDeque<u64>,
    generation: u64,
    stopping: bool,
    dropped: u64,
}

pub struct Control {
    pub robot: Robot,
    start: Instant,
    state: Mutex<State>,
    pub drive: watch::Sender<Value>,
}

impl Control {
    pub fn new(robot: Robot) -> Arc<Self> {
        Arc::new(Self {
            robot,
            start: Instant::now(),
            state: Mutex::default(),
            drive: watch::channel(Value::Null).0,
        })
    }

    fn now(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn snapshot(&self) -> (u64, String, u64) {
        let tick = self.now();
        let mut state = self.state.lock().unwrap();
        if state.ticks.len() == 10 {
            state.ticks.pop_front();
        }
        state.ticks.push_back(tick);
        if state
            .lease
            .as_ref()
            .is_some_and(|l| l.deadline <= Instant::now())
        {
            state.lease = None;
        }
        let epoch = state
            .lease
            .as_ref()
            .map(|l| l.epoch.clone())
            .unwrap_or_default();
        (tick, epoch, state.dropped)
    }

    pub fn generation(&self) -> u64 {
        self.state.lock().unwrap().generation
    }

    fn age(&self, state: &State, tick: &Value) -> u64 {
        tick.as_u64()
            .filter(|tick| state.ticks.contains(tick))
            .map(|tick| self.now().saturating_sub(tick))
            .unwrap_or(u64::MAX)
    }

    pub fn offer_drive(&self, msg: Value) {
        let mut state = self.state.lock().unwrap();
        let fresh = !state.stopping && self.age(&state, &msg["tick"]) <= 200;
        if fresh
            && let Some(lease) = &mut state.lease
            && lease.deadline > Instant::now()
            && msg["epoch"] == lease.epoch
            && let Some(seq) = msg["seq"].as_u64().filter(|seq| *seq > lease.seq)
        {
            lease.seq = seq;
            self.drive.send_replace(msg);
        } else {
            state.dropped += 1;
        }
    }

    fn prepare_drive(&self, msg: &Value) -> Option<Value> {
        let mut state = self.state.lock().unwrap();
        let age = self.age(&state, &msg["tick"]);
        if !state.stopping
            && age <= 200
            && let Some(lease) = &state.lease
            && lease.deadline > Instant::now()
            && msg["epoch"] == lease.epoch
            && let Some(mut chassis) = msg["chassis"].as_object().cloned()
        {
            chassis.insert("ttl_ms".into(), json!(200.min(300 - age)));
            return Some(json!({"token":lease.token,"targets":{"chassis":chassis}}));
        }
        state.dropped += 1;
        None
    }

    pub async fn drive_loop(&self, mut rx: watch::Receiver<Value>) {
        while rx.changed().await.is_ok() {
            let msg = rx.borrow_and_update().clone();
            if let Some(params) = self.prepare_drive(&msg)
                && self.robot.call("drive", params).await.is_err()
            {
                self.state.lock().unwrap().dropped += 1;
            }
        }
    }

    pub async fn stop(&self) -> Result<Value, ApiError> {
        let lease = {
            let mut s = self.state.lock().unwrap();
            s.generation += 1;
            s.stopping = true;
            s.lease.take()
        };
        let result = self.robot.call("stop", json!({"latch":true})).await;
        if result.is_err()
            && let Some(lease) = lease
        {
            let _ = self
                .robot
                .call("release", json!({"token":lease.token}))
                .await;
        }
        self.state.lock().unwrap().stopping = false;
        Ok(result?)
    }

    pub async fn release(&self) {
        let lease = self.state.lock().unwrap().lease.take();
        if let Some(lease) = lease {
            let _ = self
                .robot
                .call("release", json!({"token":lease.token}))
                .await;
        }
    }

    pub async fn call(
        &self,
        method: &str,
        params: Value,
        generation: u64,
    ) -> Result<Value, ApiError> {
        if matches!(method, "stop" | "drive" | "get_audio_state") {
            return Err(ApiError::new("NOT_FOUND", "未知命令"));
        }
        let mut params = params
            .as_object()
            .cloned()
            .ok_or_else(|| ApiError::new("INVALID_ARGUMENT", "params 必须是对象"))?;
        let epoch = params.remove("epoch").unwrap_or_default();
        let tick = params.remove("tick").unwrap_or_default();
        params.remove("session_id");
        params.remove("token");
        let uses_lease = matches!(
            method,
            "renew" | "release" | "targets" | "run_action" | "cancel_action" | "zero_neck"
        );
        if uses_lease || method == "acquire" {
            let state = self.state.lock().unwrap();
            if state.stopping || generation != state.generation {
                return Err(ApiError::new("FAILED_PRECONDITION", "急停已中断命令"));
            }
            if method != "release" && self.age(&state, &tick) > 500 {
                return Err(ApiError::new("STALE", "请使用最近的车端 tick"));
            }
            if uses_lease {
                let lease = state
                    .lease
                    .as_ref()
                    .filter(|l| epoch == l.epoch && l.deadline > Instant::now())
                    .ok_or_else(|| ApiError::new("PERMISSION_DENIED", "接管已失效"))?;
                params.insert("token".into(), json!(lease.token));
            }
        }
        if method == "targets"
            && params
                .get("targets")
                .and_then(|v| v.get("chassis"))
                .is_some()
        {
            return Err(ApiError::new("INVALID_ARGUMENT", "底盘目标使用 drive.v1"));
        }
        if method == "acquire" {
            params.insert("owner".into(), json!("wan/desktop"));
            params.insert("ttl_ms".into(), json!(1000));
        }
        let start = Instant::now();
        let response = self.robot.call(method, Value::Object(params)).await;
        if response
            .as_ref()
            .is_err_and(|e| uses_lease && e.code() == tonic::Code::PermissionDenied)
        {
            self.state.lock().unwrap().lease = None;
        }
        let response = response?;
        match method {
            "acquire" => {
                let token = response["token"].as_str().unwrap_or_default().to_owned();
                let epoch = Uuid::new_v4().to_string();
                let deadline = start + Duration::from_secs(1);
                {
                    let mut s = self.state.lock().unwrap();
                    if !s.stopping && generation == s.generation {
                        s.lease = Some(Lease {
                            token,
                            epoch: epoch.clone(),
                            deadline,
                            seq: 0,
                        });
                        return Ok(json!({"epoch":epoch,"ttl_ms":1000}));
                    }
                }
                let _ = self.robot.call("release", json!({"token":token})).await;
                Err(ApiError::new("FAILED_PRECONDITION", "接管已中断"))
            }
            "renew" => {
                let mut state = self.state.lock().unwrap();
                let lease = state
                    .lease
                    .as_mut()
                    .ok_or_else(|| ApiError::new("PERMISSION_DENIED", "接管已失效"))?;
                lease.deadline = start + Duration::from_secs(1);
                Ok(json!({"epoch":lease.epoch,"ttl_ms":1000}))
            }
            "release" | "update_config" => {
                self.state.lock().unwrap().lease = None;
                Ok(response)
            }
            _ => Ok(response),
        }
    }
}
