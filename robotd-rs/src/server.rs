use crate::{
    audio::StreamGuard,
    control::{Hub, Op, Reply},
    pb::{self, robot_control_server::RobotControl},
};
use std::{pin::Pin, time::Duration};
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};

#[derive(Clone)]
pub struct Service {
    pub hub: Hub,
    pub token: String,
}
macro_rules! reply {
    ($r:expr,$variant:ident) => {
        match $r {
            Reply::$variant(x) => Ok(Response::new(x)),
            _ => Err(Status::internal("内部响应类型错误")),
        }
    };
}
impl Service {
    fn auth<T>(&self, r: &Request<T>) -> Result<(), Status> {
        if self.token.is_empty() {
            return Ok(());
        }
        let expected = format!("Bearer {}", self.token);
        let supplied = r
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let difference = supplied
            .as_bytes()
            .iter()
            .zip(expected.as_bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        if supplied.len() != expected.len() || difference != 0 {
            return Err(Status::unauthenticated("访问凭据无效"));
        }
        Ok(())
    }
}
#[tonic::async_trait]
impl RobotControl for Service {
    async fn get_config(
        &self,
        r: Request<pb::Empty>,
    ) -> Result<Response<pb::ConfigSnapshot>, Status> {
        self.auth(&r)?;
        reply!(self.hub.request(Op::Config, false).await?, Config)
    }
    async fn update_config(
        &self,
        r: Request<pb::ConfigUpdate>,
    ) -> Result<Response<pb::ConfigSnapshot>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Update(r.into_inner()), false).await?,
            Config
        )
    }
    async fn get_state(&self, r: Request<pb::Empty>) -> Result<Response<pb::RobotState>, Status> {
        self.auth(&r)?;
        Ok(Response::new(self.hub.snapshot()))
    }
    type WatchStateStream = Pin<Box<dyn Stream<Item = Result<pb::RobotState, Status>> + Send>>;
    async fn watch_state(
        &self,
        r: Request<pb::WatchRequest>,
    ) -> Result<Response<Self::WatchStateStream>, Status> {
        self.auth(&r)?;
        let ms = if r.get_ref().interval_ms == 0 {
            100
        } else {
            r.get_ref().interval_ms
        };
        if !(50..=5000).contains(&ms) {
            return Err(Status::invalid_argument("interval_ms 必须为 50..5000"));
        }
        let hub = self.hub.clone();
        Ok(Response::new(Box::pin(async_stream::try_stream! {
            let mut timer=tokio::time::interval(Duration::from_millis(ms as u64));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {timer.tick().await;yield hub.snapshot();}
        })))
    }
    async fn acquire_control(
        &self,
        r: Request<pb::AcquireRequest>,
    ) -> Result<Response<pb::Lease>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Acquire(r.into_inner()), false).await?,
            Lease
        )
    }
    async fn renew_control(
        &self,
        r: Request<pb::LeaseRequest>,
    ) -> Result<Response<pb::Lease>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Renew(r.into_inner()), false).await?,
            Lease
        )
    }
    async fn release_control(
        &self,
        r: Request<pb::LeaseRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Release(r.into_inner()), true).await?,
            Ack
        )
    }
    async fn set_targets(
        &self,
        r: Request<pb::TargetRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Targets(r.into_inner()), false).await?,
            Ack
        )
    }
    async fn stream_targets(
        &self,
        r: Request<Streaming<pb::TargetRequest>>,
    ) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        let mut input = r.into_inner();
        let mut last = pb::Ack::default();
        while let Some(r) = input.message().await? {
            match self.hub.request(Op::Targets(r), false).await? {
                Reply::Ack(a) => last = a,
                _ => unreachable!(),
            }
        }
        Ok(Response::new(last))
    }
    async fn stop(&self, r: Request<pb::StopRequest>) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        reply!(self.hub.request(Op::Stop(r.into_inner()), true).await?, Ack)
    }
    async fn zero_neck(&self, r: Request<pb::ZeroRequest>) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Zero(r.into_inner()), false).await?,
            Ack
        )
    }
    async fn list_actions(
        &self,
        r: Request<pb::Empty>,
    ) -> Result<Response<pb::ActionList>, Status> {
        self.auth(&r)?;
        reply!(self.hub.request(Op::Actions, false).await?, Actions)
    }
    async fn put_action(
        &self,
        r: Request<pb::ActionUpdate>,
    ) -> Result<Response<pb::ActionRecord>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Put(r.into_inner()), false).await?,
            Action
        )
    }
    async fn delete_action(
        &self,
        r: Request<pb::ActionDelete>,
    ) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Delete(r.into_inner()), false).await?,
            Ack
        )
    }
    async fn run_action(&self, r: Request<pb::RunRequest>) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        reply!(self.hub.request(Op::Run(r.into_inner()), false).await?, Ack)
    }
    async fn cancel_action(
        &self,
        r: Request<pb::LeaseRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        self.auth(&r)?;
        reply!(
            self.hub.request(Op::Cancel(r.into_inner()), true).await?,
            Ack
        )
    }
    async fn get_audio_state(
        &self,
        r: Request<pb::Empty>,
    ) -> Result<Response<pb::AudioStatus>, Status> {
        self.auth(&r)?;
        Ok(Response::new(self.hub.audio.status()))
    }
    async fn stop_audio(&self, r: Request<pb::Empty>) -> Result<Response<pb::AudioStatus>, Status> {
        self.auth(&r)?;
        Ok(Response::new(self.hub.audio.stop()))
    }
    async fn play_audio(
        &self,
        r: Request<Streaming<pb::AudioPacket>>,
    ) -> Result<Response<pb::AudioStatus>, Status> {
        self.auth(&r)?;
        let mut input = r.into_inner();
        let header = match input.message().await?.and_then(|m| m.payload) {
            Some(pb::audio_packet::Payload::Header(h)) => h,
            _ => return Err(Status::invalid_argument("音频首包必须是 header")),
        };
        let s = match self.hub.request(Op::Audio(header), false).await? {
            Reply::Audio(s) => s,
            _ => unreachable!(),
        };
        // RPC 被取消或上游报错时，RAII 同时清空排队音频，不留下后台续播。
        let _guard = StreamGuard(s.clone());
        loop {
            // 上游可能停在生成阶段，停止不能等待下一包音频才结束 RPC。
            let packet = tokio::select! {
                packet = input.message() => packet?,
                error = s.interrupted() => return Err(error),
            };
            let Some(packet) = packet else { break };
            match packet.payload {
                Some(pb::audio_packet::Payload::Pcm(bytes)) => s.push(&bytes).await?,
                _ => return Err(Status::invalid_argument("音频 header 只能出现在首包")),
            }
        }
        Ok(Response::new(s.finish().await?))
    }
}
