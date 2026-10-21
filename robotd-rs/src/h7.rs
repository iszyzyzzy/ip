//! 直接复用 H7 的 COBS/CRC CDC 协议；超时不重发有副作用的命令。
use anyhow::{Result, bail, ensure};
use serialport::SerialPort;
use std::{
    collections::VecDeque,
    io::{Read, Write},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub kind: u8,
    pub channel: u8,
    pub flags: u8,
    pub seq: u16,
    pub payload: Vec<u8>,
}
pub fn crc(data: &[u8]) -> u16 {
    let mut c = 0xffffu16;
    for b in data {
        c ^= (*b as u16) << 8;
        for _ in 0..8 {
            c = (c << 1) ^ if c & 0x8000 != 0 { 0x1021 } else { 0 };
        }
    }
    c
}
impl Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut raw = vec![1, self.kind, self.channel, self.flags];
        raw.extend(self.seq.to_le_bytes());
        raw.extend((self.payload.len() as u16).to_le_bytes());
        raw.extend(&self.payload);
        raw.extend(crc(&raw).to_le_bytes());
        let mut out = vec![0];
        let (mut pos, mut code) = (0, 1u8);
        for b in raw {
            if b == 0 {
                out[pos] = code;
                pos = out.len();
                out.push(0);
                code = 1;
            } else {
                out.push(b);
                code += 1;
                if code == 255 {
                    out[pos] = code;
                    pos = out.len();
                    out.push(0);
                    code = 1;
                }
            }
        }
        out[pos] = code;
        out.push(0);
        out
    }
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut raw = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            let c = data[pos] as usize;
            pos += 1;
            ensure!(c > 0 && pos + c - 1 <= data.len(), "COBS 长度错误");
            raw.extend(&data[pos..pos + c - 1]);
            pos += c - 1;
            if c != 255 && pos < data.len() {
                raw.push(0);
            }
        }
        ensure!(raw.len() >= 10 && raw[0] == 1, "CDC 版本或长度错误");
        let len = u16::from_le_bytes([raw[6], raw[7]]) as usize;
        ensure!(
            len <= 80
                && raw.len() == len + 10
                && crc(&raw[..raw.len() - 2])
                    == u16::from_le_bytes([raw[raw.len() - 2], raw[raw.len() - 1]]),
            "CDC 长度或 CRC 错误"
        );
        Ok(Self {
            kind: raw[1],
            channel: raw[2],
            flags: raw[3],
            seq: u16::from_le_bytes([raw[4], raw[5]]),
            payload: raw[8..8 + len].to_vec(),
        })
    }
}
pub struct Bridge {
    pub port: Box<dyn SerialPort>,
    pub frames: VecDeque<Packet>,
    pub dropped: u64,
    inbox: VecDeque<Packet>,
    wire: Vec<u8>,
    discarding: bool,
    seq: u16,
    timeout: Duration,
}
impl Bridge {
    pub fn open(cfg: &crate::config::H7) -> Result<Self> {
        let mut port = serialport::new(&cfg.port, 115200)
            .timeout(Duration::from_millis(5))
            .open()?;
        port.write_data_terminal_ready(true)?;
        port.write_all(&[0])?;
        Ok(Self {
            port,
            frames: VecDeque::new(),
            dropped: 0,
            inbox: VecDeque::new(),
            wire: Vec::with_capacity(92),
            discarding: false,
            seq: 0,
            timeout: Duration::from_millis(cfg.timeout_ms),
        })
    }
    pub fn feed(&mut self, data: &[u8]) {
        for &b in data {
            if b == 0 {
                if !self.discarding
                    && !self.wire.is_empty()
                    && let Ok(p) = Packet::decode(&self.wire)
                {
                    let limit = if p.kind == 0x81 { 512 } else { 64 };
                    let queue = if p.kind == 0x81 {
                        &mut self.frames
                    } else {
                        &mut self.inbox
                    };
                    if queue.len() >= limit {
                        queue.pop_front();
                        self.dropped += 1;
                    }
                    queue.push_back(p);
                }
                self.wire.clear();
                self.discarding = false;
            } else if !self.discarding {
                if self.wire.len() >= 92 {
                    self.discarding = true;
                } else {
                    self.wire.push(b);
                }
            }
        }
    }
    pub fn poll(&mut self) -> Result<()> {
        if self.port.bytes_to_read()? == 0 {
            return Ok(());
        }
        let mut buf = [0u8; 4096];
        match self.port.read(&mut buf) {
            Ok(n) => self.feed(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }
    fn wait(&mut self, kind: u8, channel: u8, seq: u16) -> Result<Packet> {
        let end = Instant::now() + self.timeout;
        loop {
            if let Some(i) = self
                .inbox
                .iter()
                .position(|p| p.kind == kind && p.channel == channel && p.seq == seq)
            {
                return Ok(self.inbox.remove(i).unwrap());
            }
            ensure!(
                Instant::now() < end,
                "H7 响应超时，未重发：type={kind:#x}, seq={seq}"
            );
            self.poll()?;
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn request(&mut self, kind: u8, channel: u8, payload: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;
        self.port.write_all(
            &Packet {
                kind,
                channel,
                flags: 0,
                seq,
                payload,
            }
            .encode(),
        )?;
        let p = self.wait(0x80, channel, seq)?;
        ensure!(
            p.payload.len() >= 2 && p.payload[0] == kind,
            "H7 响应不匹配"
        );
        if p.payload[1] != 0 {
            bail!("H7 命令失败：状态 {}", p.payload[1]);
        }
        Ok((seq, p.payload[2..].to_vec()))
    }
    pub fn configure(&mut self, channel: u8, rate: u32) -> Result<()> {
        self.request(3, channel, vec![0])?;
        let mut p = rate.to_le_bytes().to_vec();
        p.extend(2000000u32.to_le_bytes());
        p.push(0);
        self.request(2, channel, p)?;
        self.request(3, channel, vec![1])?;
        Ok(())
    }
    pub fn send(&mut self, channel: u8, id: u32, bytes: &[u8]) -> Result<()> {
        let mut p = id.to_le_bytes().to_vec();
        p.push(bytes.len() as u8);
        p.extend(bytes);
        let (seq, _) = self.request(4, channel, p)?;
        let p = self.wait(0x82, channel, seq)?;
        ensure!(
            p.payload.len() == 5 && p.payload[0] == 0,
            "H7 CAN 实际发送失败"
        );
        Ok(())
    }
    pub fn motor_stop(&mut self, mask: u8) -> Result<()> {
        self.request(0x12, 1, vec![mask])?;
        Ok(())
    }
    pub fn motor_zero(&mut self, mask: u8) -> Result<()> {
        self.motor_stop(mask)?;
        self.request(0x14, 1, vec![mask])?;
        Ok(())
    }
    pub fn motor_tune(&mut self, motor: u8, t: &crate::config::Tuning) -> Result<()> {
        let mut p = vec![motor];
        for x in [
            t.position_kp,
            t.velocity_kp,
            t.velocity_ki,
            t.max_velocity,
            t.max_output,
            t.max_position,
        ] {
            p.extend((x as f32).to_le_bytes());
        }
        self.request(0x13, 1, p)?;
        Ok(())
    }
    pub fn motor_position(&mut self, gm: Option<f64>, dm: Option<f64>) -> Result<()> {
        let mut p = vec![u8::from(gm.is_some()) | (u8::from(dm.is_some()) << 1)];
        p.extend((gm.unwrap_or(0.0) as f32).to_le_bytes());
        p.extend((dm.unwrap_or(0.0) as f32).to_le_bytes());
        self.request(0x10, 1, p)?;
        Ok(())
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.port.write_data_terminal_ready(false);
    }
}
