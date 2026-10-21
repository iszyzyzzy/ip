//! 语音流独占一个小队列；音频阻塞与设备控制线程分离。
use crate::{config::Audio, pb};
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Notify;
use tonic::Status;

#[derive(Clone)]
pub struct Player(Arc<Mutex<Manager>>);
struct Manager {
    cfg: Audio,
    active: Option<Arc<Session>>,
    simulated: bool,
}
struct State {
    queue: VecDeque<u8>,
    eof: bool,
    cancelled: bool,
    done: bool,
    received: u64,
    status: pb::AudioStatus,
}
pub struct Session {
    state: Mutex<State>,
    ready: Condvar,
    changed: Notify,
    cfg: Audio,
    rate: u32,
    channels: u32,
    created: Instant,
    first_pcm: Mutex<Option<Instant>>,
}

impl Player {
    pub fn new(cfg: Audio, simulated: bool) -> Self {
        Self(Arc::new(Mutex::new(Manager {
            cfg,
            active: None,
            simulated,
        })))
    }
    pub fn configure(&self, cfg: Audio) {
        let mut m = self.0.lock().unwrap();
        if let Some(s) = &m.active {
            s.cancel();
        }
        m.cfg = cfg;
    }
    pub fn status(&self) -> pb::AudioStatus {
        let m = self.0.lock().unwrap();
        m.active
            .as_ref()
            .map(|s| s.status())
            .unwrap_or_else(|| pb::AudioStatus {
                phase: if m.cfg.enabled { "idle" } else { "disabled" }.into(),
                simulated: m.simulated,
                ..Default::default()
            })
    }
    pub fn stop(&self) -> pb::AudioStatus {
        let m = self.0.lock().unwrap();
        if let Some(s) = &m.active {
            s.cancel();
        }
        drop(m);
        self.status()
    }
    pub fn start(&self, h: pb::AudioHeader) -> Result<Arc<Session>, Status> {
        if ![
            8000, 16000, 22050, 24000, 32000, 44100, 48000, 88200, 96000, 192000,
        ]
        .contains(&h.sample_rate_hz)
            || ![1, 2].contains(&h.channels)
        {
            return Err(Status::invalid_argument("PCM16 需要有效采样率和 1/2 声道"));
        }
        let mut m = self.0.lock().unwrap();
        if !m.cfg.enabled {
            return Err(Status::failed_precondition("音频未启用，请更新 audio 配置"));
        }
        if m.active
            .as_ref()
            .is_some_and(|s| !s.state.lock().unwrap().done)
        {
            return Err(Status::resource_exhausted("已有音频流正在输入或播放"));
        }
        let capacity =
            (h.sample_rate_hz as usize * h.channels as usize * 2 * m.cfg.max_buffer_ms as usize)
                / 1000;
        let s = Arc::new(Session {
            state: Mutex::new(State {
                queue: VecDeque::with_capacity(capacity),
                eof: false,
                cancelled: false,
                done: false,
                received: 0,
                status: pb::AudioStatus {
                    stream_id: uuid::Uuid::new_v4().to_string(),
                    phase: "buffering".into(),
                    simulated: m.simulated,
                    sample_rate_hz: h.sample_rate_hz,
                    channels: h.channels,
                    ..Default::default()
                },
            }),
            ready: Condvar::new(),
            changed: Notify::new(),
            cfg: m.cfg.clone(),
            rate: h.sample_rate_hz,
            channels: h.channels,
            created: Instant::now(),
            first_pcm: Mutex::new(None),
        });
        let worker = s.clone();
        let simulated = m.simulated;
        std::thread::Builder::new()
            .name("robot-audio".into())
            .stack_size(256 * 1024)
            .spawn(move || worker.run(simulated))
            .map_err(|e| Status::internal(e.to_string()))?;
        m.active = Some(s.clone());
        Ok(s)
    }
}

impl Session {
    fn frame_bytes(&self) -> usize {
        self.channels as usize * 2
    }
    fn bytes_ms(&self, ms: u32) -> usize {
        self.rate as usize * ms as usize / 1000 * self.frame_bytes()
    }
    fn ms(&self, n: usize) -> f64 {
        n as f64 * 1000.0 / (self.rate as f64 * self.frame_bytes() as f64)
    }
    pub fn status(&self) -> pb::AudioStatus {
        let s = self.state.lock().unwrap();
        let mut result = s.status.clone();
        result.buffered_ms = self.ms(s.queue.len());
        result.received_frames = s.received / self.frame_bytes() as u64;
        result
    }
    fn check(s: &State) -> Result<(), Status> {
        if !s.status.error.is_empty() {
            return Err(Status::unavailable(s.status.error.clone()));
        }
        if s.cancelled {
            return Err(Status::cancelled("音频已停止"));
        }
        if s.done || s.eof {
            return Err(Status::failed_precondition("音频流已结束"));
        }
        Ok(())
    }
    pub async fn push(&self, bytes: &[u8]) -> Result<(), Status> {
        if bytes.len() > 65536 {
            return Err(Status::invalid_argument("每包 PCM 最多 64 KiB"));
        }
        let mut offset = 0;
        while offset < bytes.len() {
            // 先注册通知再检查队列，避免消费者恰好取走数据时丢失唤醒。
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let n = {
                let mut s = self.state.lock().unwrap();
                Self::check(&s)?;
                let n = (self.bytes_ms(self.cfg.max_buffer_ms) - s.queue.len())
                    .min(bytes.len() - offset);
                if n > 0 {
                    let mut first = self.first_pcm.lock().unwrap();
                    if first.is_none() {
                        *first = Some(Instant::now());
                    }
                    s.queue.extend(&bytes[offset..offset + n]);
                    s.received += n as u64;
                    s.status.received_frames = s.received / self.frame_bytes() as u64;
                    s.status.peak_buffered_ms =
                        s.status.peak_buffered_ms.max(self.ms(s.queue.len()));
                    self.ready.notify_one();
                }
                n
            };
            offset += n;
            if n == 0 {
                changed.await;
            }
        }
        Ok(())
    }
    pub async fn finish(&self) -> Result<pb::AudioStatus, Status> {
        {
            let mut s = self.state.lock().unwrap();
            Self::check(&s)?;
            if !s.received.is_multiple_of(self.frame_bytes() as u64) {
                return Err(Status::invalid_argument("PCM 尾部不足一个完整采样帧"));
            }
            s.eof = true;
            self.ready.notify_one();
        }
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let s = self.state.lock().unwrap();
                if s.done {
                    if !s.status.error.is_empty() {
                        return Err(Status::unavailable(s.status.error.clone()));
                    }
                    if s.cancelled {
                        return Err(Status::cancelled("音频已停止"));
                    }
                    return Ok(s.status.clone());
                }
            }
            changed.await;
        }
    }
    pub async fn interrupted(&self) -> Status {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let s = self.state.lock().unwrap();
                if let Err(e) = Self::check(&s) {
                    return e;
                }
            }
            changed.await;
        }
    }
    pub fn cancel(&self) {
        let mut s = self.state.lock().unwrap();
        if !s.done {
            s.cancelled = true;
            s.queue.clear();
            s.status.phase = "stopping".into();
            self.ready.notify_all();
            self.changed.notify_waiters();
        }
    }
    fn cancelled(&self) -> bool {
        self.state.lock().unwrap().cancelled
    }
    fn pause(&self, d: Duration) {
        let s = self.state.lock().unwrap();
        if !s.cancelled {
            drop(self.ready.wait_timeout(s, d).unwrap());
        }
    }
    fn run(&self, simulated: bool) {
        let result = self.play(simulated);
        let mut s = self.state.lock().unwrap();
        if let Err(e) = result
            && !s.cancelled
        {
            s.status.error = e.to_string();
        }
        s.queue.clear();
        s.done = true;
        s.status.phase = if s.cancelled {
            "cancelled"
        } else if s.status.error.is_empty() {
            "completed"
        } else {
            "failed"
        }
        .into();
        self.changed.notify_waiters();
        self.ready.notify_all();
    }
    fn play(&self, simulated: bool) -> anyhow::Result<()> {
        let mut sink: Option<Sink> = None;
        let mut priming = true;
        let mut output_at = Instant::now();
        let block_size = self.bytes_ms(10);
        loop {
            let (bytes, silence) = {
                let mut s = self.state.lock().unwrap();
                loop {
                    if s.cancelled {
                        return Ok(());
                    }
                    if s.eof && s.queue.is_empty() {
                        drop(s);
                        if let Some(sink) = &mut sink {
                            sink.drain(self)?;
                        }
                        return Ok(());
                    }
                    let threshold = self.bytes_ms(if sink.is_none() {
                        self.cfg.prebuffer_ms
                    } else {
                        self.cfg.rebuffer_ms
                    });
                    if priming && !s.eof && s.queue.len() < threshold {
                        if sink.is_none() {
                            s = self.ready.wait(s).unwrap();
                            continue;
                        }
                        s.status.phase = "rebuffering".into();
                        break (vec![0; block_size], true);
                    }
                    priming = false;
                    let n = s.queue.len().min(block_size) / self.frame_bytes() * self.frame_bytes();
                    if n == 0 {
                        priming = true;
                        s.status.rebuffer_count += 1;
                        continue;
                    }
                    s.status.phase = if s.eof { "draining" } else { "playing" }.into();
                    let bytes: Vec<u8> = s.queue.drain(..n).collect();
                    self.changed.notify_waiters();
                    break (bytes, false);
                }
            };
            if sink.is_none() {
                sink = Some(Sink::open(&self.cfg, self.rate, self.channels, simulated)?);
                let mut s = self.state.lock().unwrap();
                s.status.device_buffer_ms = sink.as_ref().unwrap().buffer_ms();
                s.status.start_latency_ms = self
                    .first_pcm
                    .lock()
                    .unwrap()
                    .unwrap_or(self.created)
                    .elapsed()
                    .as_secs_f64()
                    * 1000.0;
                output_at = Instant::now();
            }
            if self.cancelled() {
                return Ok(());
            }
            sink.as_mut().unwrap().write(&bytes, self)?;
            {
                let mut s = self.state.lock().unwrap();
                let frames = (bytes.len() / self.frame_bytes()) as u64;
                if silence {
                    s.status.silence_frames += frames;
                } else {
                    s.status.submitted_frames += frames;
                }
            }
            // null 声卡也必须按采样率节流，否则静音填充会占满 CPU。
            output_at += Duration::from_secs_f64(self.ms(bytes.len()) / 1000.0);
            while !self.cancelled() && Instant::now() < output_at {
                self.pause(output_at.saturating_duration_since(Instant::now()));
            }
            if Instant::now().duration_since(output_at) > Duration::from_millis(20) {
                output_at = Instant::now();
            }
        }
    }
}

enum Sink {
    Simulated,
    #[cfg(target_os = "linux")]
    Alsa {
        pcm: alsa::pcm::PCM,
        buffer_ms: f64,
        channels: usize,
    },
}
impl Sink {
    fn open(cfg: &Audio, rate: u32, channels: u32, simulated: bool) -> anyhow::Result<Self> {
        if simulated {
            return Ok(Self::Simulated);
        }
        #[cfg(target_os = "linux")]
        {
            use alsa::{
                Direction, ValueOr,
                pcm::{Access, Format, HwParams, PCM},
            };
            let pcm = PCM::new(&cfg.device, Direction::Playback, true)?;
            let h = HwParams::any(&pcm)?;
            h.set_access(Access::RWInterleaved)?;
            h.set_format(Format::S16LE)?;
            h.set_channels(channels)?;
            h.set_rate(rate, ValueOr::Nearest)?;
            let actual_rate = h.get_rate()?;
            anyhow::ensure!(
                actual_rate == rate,
                "ALSA 采样率不匹配，请使用 plughw/plug 设备转换"
            );
            h.set_buffer_time_near(cfg.device_buffer_ms * 1000, ValueOr::Nearest)?;
            h.set_period_time_near(10000, ValueOr::Nearest)?;
            pcm.hw_params(&h)?;
            drop(h);
            let (frames, period) = pcm.get_params()?;
            let sw = pcm.sw_params_current()?;
            // 留出完整设备缓冲抵抗调度抖动，不能每次仅剩一个 period 才补数据。
            sw.set_start_threshold(frames.try_into()?)?;
            sw.set_avail_min(period.try_into()?)?;
            pcm.sw_params(&sw)?;
            drop(sw);
            let buffer_ms = frames as f64 * 1000.0 / rate as f64;
            anyhow::ensure!(
                buffer_ms + cfg.max_buffer_ms as f64 <= 200.5,
                "声卡实际缓冲 {buffer_ms:.1} ms，加软件队列超过 200 ms，请调整 ALSA 设备或缓冲配置"
            );
            Ok(Self::Alsa {
                pcm,
                buffer_ms,
                channels: channels as usize,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (cfg, rate, channels);
            anyhow::bail!("ALSA 播放需要 Linux；本机开发请使用 --simulate");
        }
    }
    fn buffer_ms(&self) -> f64 {
        match self {
            Self::Simulated => 0.0,
            #[cfg(target_os = "linux")]
            Self::Alsa { buffer_ms, .. } => *buffer_ms,
        }
    }
    fn write(&mut self, bytes: &[u8], session: &Session) -> anyhow::Result<()> {
        match self {
            Self::Simulated => {
                let _ = (bytes, session);
                Ok(())
            }
            #[cfg(target_os = "linux")]
            Self::Alsa { pcm, channels, .. } => {
                let samples: Vec<i16> = bytes
                    .chunks_exact(2)
                    .map(|x| i16::from_le_bytes([x[0], x[1]]))
                    .collect();
                let io = pcm.io_i16()?;
                let mut offset = 0;
                while offset < samples.len() && !session.cancelled() {
                    match io.writei(&samples[offset..]) {
                        Ok(n) => {
                            offset += n * *channels;
                            if n == 0 {
                                session.pause(Duration::from_millis(2));
                            }
                        }
                        Err(e) if e.errno() == libc::EAGAIN => {
                            session.pause(Duration::from_millis(2))
                        }
                        Err(e) => pcm.try_recover(e, true)?,
                    }
                }
                Ok(())
            }
        }
    }
    fn drain(&mut self, session: &Session) -> anyhow::Result<()> {
        #[cfg(target_os = "linux")]
        if let Self::Alsa { pcm, .. } = self {
            if pcm.state() == alsa::pcm::State::Prepared {
                pcm.start()?;
            }
            while !session.cancelled() {
                match pcm.drain() {
                    Ok(()) => break,
                    Err(e) if e.errno() == libc::EAGAIN => session.pause(Duration::from_millis(2)),
                    Err(e) => return Err(e.into()),
                }
            }
        }
        let _ = session;
        Ok(())
    }
}
impl Drop for Sink {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Self::Alsa { pcm, .. } = self {
            let _ = alsa::pcm::PCM::drop(pcm);
        }
    }
}

pub struct StreamGuard(pub Arc<Session>);
impl Drop for StreamGuard {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
