//! gRPC transport is independent of the UI. Commands are acknowledged only once
//! the render thread has applied them. A bounded mailbox provides backpressure.
use std::{
    io,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::{mpsc, oneshot},
};
use tonic::{Request, Response, Status};
pub mod pb {
    tonic::include_proto!("eyes.v1");
}

pub enum Command {
    Pose(pb::Pose),
    Configure(Box<pb::Config>),
    Blink,
    ReloadAssets,
    SaveSettings,
    ConnectTracking,
    TriggerBlink(pb::BlinkRequest),
}
pub struct Pending {
    pub command: Command,
    pub reply: oneshot::Sender<Result<u64, String>>,
}
pub struct ControlHub {
    pub commands: mpsc::Receiver<Pending>,
    pub state: Arc<Mutex<pb::State>>,
    runtime: Option<tokio::runtime::Runtime>,
    path: PathBuf,
}
#[derive(Clone)]
struct Service {
    tx: mpsc::Sender<Pending>,
    state: Arc<Mutex<pb::State>>,
}
impl Service {
    async fn apply(&self, command: Command) -> Result<Response<pb::Ack>, Status> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .try_send(Pending { command, reply })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => {
                    Status::resource_exhausted("render command queue is full")
                }
                mpsc::error::TrySendError::Closed(_) => Status::unavailable("renderer stopped"),
            })?;
        let revision = tokio::time::timeout(Duration::from_secs(3), rx)
            .await
            .map_err(|_| {
                Status::deadline_exceeded("renderer did not apply command within 3 seconds")
            })?
            .map_err(|_| Status::unavailable("renderer stopped"))?
            .map_err(Status::failed_precondition)?;
        Ok(Response::new(pb::Ack { revision }))
    }
}
#[tonic::async_trait]
impl pb::eye_control_server::EyeControl for Service {
    async fn get_state(&self, _: Request<pb::Empty>) -> Result<Response<pb::State>, Status> {
        Ok(Response::new(
            self.state
                .lock()
                .map_err(|_| Status::internal("state lock"))?
                .clone(),
        ))
    }
    async fn set_pose(&self, request: Request<pb::Pose>) -> Result<Response<pb::Ack>, Status> {
        let pose = request.into_inner();
        validate_pose(&pose)?;
        self.apply(Command::Pose(pose)).await
    }
    async fn stream_poses(
        &self,
        request: Request<tonic::Streaming<pb::Pose>>,
    ) -> Result<Response<pb::Ack>, Status> {
        let mut stream = request.into_inner();
        let mut ack = pb::Ack {
            revision: self
                .state
                .lock()
                .map_err(|_| Status::internal("state lock"))?
                .revision,
        };
        while let Some(pose) = stream.message().await? {
            validate_pose(&pose)?;
            ack = self.apply(Command::Pose(pose)).await?.into_inner();
        }
        Ok(Response::new(ack))
    }
    async fn configure(&self, request: Request<pb::Config>) -> Result<Response<pb::Ack>, Status> {
        let config = request.into_inner();
        validate_config(&config)?;
        self.apply(Command::Configure(Box::new(config))).await
    }
    async fn blink(&self, _: Request<pb::Empty>) -> Result<Response<pb::Ack>, Status> {
        self.apply(Command::Blink).await
    }
    async fn reload_assets(&self, _: Request<pb::Empty>) -> Result<Response<pb::Ack>, Status> {
        self.apply(Command::ReloadAssets).await
    }
    async fn save_settings(&self, _: Request<pb::Empty>) -> Result<Response<pb::Ack>, Status> {
        self.apply(Command::SaveSettings).await
    }
    async fn connect_tracking(&self, _: Request<pb::Empty>) -> Result<Response<pb::Ack>, Status> {
        self.apply(Command::ConnectTracking).await
    }
    async fn trigger_blink(
        &self,
        r: Request<pb::BlinkRequest>,
    ) -> Result<Response<pb::Ack>, Status> {
        let r = r.into_inner();
        if let Some(d) = r.duration {
            range(d, 0.08, 1., "blink duration")?;
        }
        if !r.left.unwrap_or(true) && !r.right.unwrap_or(true) {
            return Err(Status::invalid_argument("select at least one eye"));
        }
        self.apply(Command::TriggerBlink(r)).await
    }
}
fn range(v: f32, lo: f32, hi: f32, name: &str) -> Result<(), Status> {
    if v.is_finite() && (lo..=hi).contains(&v) {
        Ok(())
    } else {
        Err(Status::invalid_argument(format!(
            "{name} must be finite and in [{lo}, {hi}]"
        )))
    }
}
pub fn validate_pose(p: &pb::Pose) -> Result<(), Status> {
    for eye in [&p.left, &p.right] {
        let eye = eye
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("both left and right eyes are required"))?;
        range(eye.gaze_x, -1., 1., "gaze_x")?;
        range(eye.gaze_y, -1., 1., "gaze_y")?;
        range(eye.openness, 0., 1.4, "openness")?;
        range(eye.brow, -1., 1., "brow")?;
    }
    range(p.roll, -0.8, 0.8, "roll")
}
pub fn validate_config(c: &pb::Config) -> Result<(), Status> {
    if c.mode.is_some_and(|m| pb::Mode::try_from(m).is_err()) {
        return Err(Status::invalid_argument("unknown mode"));
    }
    for (v, lo, hi, name) in [
        (c.scale, 0.5, 1.5, "scale"),
        (c.spacing, 0.38, 0.7, "spacing"),
        (c.iris_size, 0.1, 0.22, "iris_size"),
        (c.pupil, 0.2, 0.65, "pupil"),
        (c.tilt, -0.2, 0.3, "tilt"),
        (c.glow, 0., 1., "glow"),
        (c.expression, -1., 1., "expression"),
        (c.response, 3., 30., "response"),
        (c.blink_interval, 0.5, 20., "blink_interval"),
        (c.blink_duration, 0.08, 1., "blink_duration"),
        (c.blink_variation, 0., 0.8, "blink_variation"),
    ] {
        if let Some(v) = v {
            range(v, lo, hi, name)?;
        }
    }
    for color in [&c.iris, &c.background].into_iter().flatten() {
        for v in [color.r, color.g, color.b] {
            range(v, 0., 1., "color")?;
        }
    }
    if let Some(j) = &c.jelly {
        for (v, lo, hi, name) in [
            (j.strength, 0., 1., "jelly.strength"),
            (j.frequency, 1., 12., "jelly.frequency"),
            (j.damping, 0.15, 1.5, "jelly.damping"),
        ] {
            if let Some(v) = v {
                range(v, lo, hi, name)?;
            }
        }
    }
    if let Some(s) = &c.simulator {
        for (v, lo, hi, name) in [
            (s.target_x, -1., 1., "target_x"),
            (s.target_y, -1., 1., "target_y"),
            (s.motion_amplitude, 0., 0.3, "motion_amplitude"),
            (s.motion_speed, 0.1, 8., "motion_speed"),
            (s.convergence, 0., 0.3, "convergence"),
            (s.left_openness, 0., 1.4, "left_openness"),
            (s.right_openness, 0., 1.4, "right_openness"),
            (s.left_brow, -1., 1., "left_brow"),
            (s.right_brow, -1., 1., "right_brow"),
            (s.roll, -0.8, 0.8, "simulator.roll"),
        ] {
            if let Some(v) = v {
                range(v, lo, hi, name)?;
            }
        }
    }
    if let Some(a) = &c.art {
        for (v, lo, hi, name) in [
            (a.half_width, 0.2, 0.36, "half_width"),
            (a.upper_height, 0.08, 0.3, "upper_height"),
            (a.lower_height, 0.06, 0.28, "lower_height"),
            (a.roundness, 0.5, 1.5, "roundness"),
            (a.iris_aspect, 0.6, 1.2, "iris_aspect"),
            (a.brow_height, 0.22, 0.5, "brow_height"),
            (a.lid_texture_height, 0.04, 0.25, "lid_texture_height"),
            (a.shadow_strength, 0., 0.5, "shadow_strength"),
        ] {
            if let Some(v) = v {
                range(v, lo, hi, name)?;
            }
        }
    }
    if c.tracking_port
        .is_some_and(|p| !(1024..=65535).contains(&p))
    {
        return Err(Status::invalid_argument(
            "tracking_port must be 1024..65535",
        ));
    }
    if let Some(ip) = &c.phone_ip
        && !ip.is_empty()
        && ip.parse::<std::net::Ipv4Addr>().is_err()
    {
        return Err(Status::invalid_argument("phone_ip must be IPv4 or empty"));
    }
    Ok(())
}

// Tonic needs Connected in addition to Tokio's IO traits for custom transports.
struct Stream(uds_compat::UnixStream);
impl tonic::transport::server::Connected for Stream {
    type ConnectInfo = ();
    fn connect_info(&self) {}
}
impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // uds-compat 0.1's Windows adapter expects an initially empty ReadBuf.
        let mut fresh = ReadBuf::new(buf.initialize_unfilled());
        match Pin::new(&mut self.get_mut().0).poll_read(cx, &mut fresh) {
            Poll::Ready(Ok(())) => {
                let count = fresh.filled().len();
                buf.advance(count);
                Poll::Ready(Ok(()))
            }
            result => result,
        }
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}
impl ControlHub {
    pub fn watch_interrupt(&self, stopping: Arc<std::sync::atomic::AtomicBool>) {
        self.runtime.as_ref().unwrap().spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                stopping.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
    }
    pub fn start(path: PathBuf) -> io::Result<Self> {
        // Never delete a pre-existing endpoint: it could belong to another app.
        if std::fs::symlink_metadata(&path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} already exists; choose another --socket path or remove a stale endpoint after stopping its owner",
                    path.display()
                ),
            ));
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let listener = {
            let _guard = runtime.enter();
            uds_compat::UnixListener::bind(&path)?
        };
        let (tx, commands) = mpsc::channel(64);
        let state = Arc::new(Mutex::new(pb::State::default()));
        let service = Service {
            tx,
            state: state.clone(),
        };
        runtime.spawn(async move {
            let incoming = async_stream::stream! {
                loop { match listener.accept().await { Ok((stream, _)) => yield Ok::<_, io::Error>(Stream(stream)), Err(e) => { yield Err(e); break; } } }
            };
            if let Err(e) = tonic::transport::Server::builder()
                .add_service(pb::eye_control_server::EyeControlServer::new(service).max_decoding_message_size(16 * 1024))
                .serve_with_incoming(incoming).await { eprintln!("gRPC server stopped: {e}"); }
        });
        Ok(Self {
            commands,
            state,
            runtime: Some(runtime),
            path,
        })
    }
}
impl Drop for ControlHub {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_millis(300));
        }
        let _ = std::fs::remove_file(&self.path);
    }
}
/// Example clients can reuse this connector; the dummy HTTP authority is never
/// resolved or contacted. All bytes travel through AF_UNIX.
pub async fn connect(
    path: impl AsRef<Path>,
) -> Result<
    pb::eye_control_client::EyeControlClient<tonic::transport::Channel>,
    tonic::transport::Error,
> {
    let path = path.as_ref().to_path_buf();
    let channel = tonic::transport::Endpoint::from_static("http://[::]:50051")
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .connect_with_connector(tower::service_fn(move |_| {
            let path = path.clone();
            async move {
                uds_compat::UnixStream::connect(path)
                    .await
                    .map(|s| hyper_util::rt::TokioIo::new(Stream(s)))
            }
        }))
        .await?;
    Ok(pb::eye_control_client::EyeControlClient::new(channel))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_controls() {
        assert!(validate_pose(&pb::Pose::default()).is_err());
        assert!(
            validate_config(&pb::Config {
                scale: Some(f32::NAN),
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_config(&pb::Config {
                mode: Some(99),
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_config(&pb::Config {
                expression: Some(-1.),
                ..Default::default()
            })
            .is_ok()
        );
    }
    #[test]
    fn real_grpc_over_unix_socket() {
        let path = std::env::temp_dir().join(format!("eyes-test-{}.sock", std::process::id()));
        let mut hub = ControlHub::start(path.clone()).unwrap();
        assert!(ControlHub::start(path.clone()).is_err());
        let client_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut client = connect(path).await.unwrap();
                assert_eq!(
                    client
                        .get_state(pb::Empty {})
                        .await
                        .unwrap()
                        .into_inner()
                        .revision,
                    0
                );
                assert_eq!(
                    client
                        .set_pose(pb::Pose::default())
                        .await
                        .unwrap_err()
                        .code(),
                    tonic::Code::InvalidArgument
                );
                let ack = client.blink(pb::Empty {}).await.unwrap().into_inner();
                assert_eq!(ack.revision, 42);
            });
        });
        let start = std::time::Instant::now();
        loop {
            if let Ok(pending) = hub.commands.try_recv() {
                assert!(matches!(pending.command, Command::Blink));
                pending.reply.send(Ok(42)).unwrap();
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        client_thread.join().unwrap();
        let path = hub.path.clone();
        drop(hub);
        assert!(std::fs::symlink_metadata(path).is_err());
    }
}
