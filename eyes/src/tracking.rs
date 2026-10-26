//! iFacialMocap UDP v1/v2. Networking stays on the UI thread but is nonblocking
//! and bounded, so neither an absent phone nor a packet flood can stall rendering.
use std::{
    io,
    net::{IpAddr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct Face {
    pub blink: [f32; 2],
    pub wide: [f32; 2],
    pub squint: [f32; 2],
    pub gaze: [[f32; 2]; 2],
    pub brow: [f32; 2],
    pub roll: f32,
}

/// Every datagram is a complete sample. Missing fields return to neutral;
/// nonfinite, unknown and malformed values never reach the renderer.
pub fn parse(packet: &[u8]) -> Option<Face> {
    let text = std::str::from_utf8(packet).ok()?;
    let mut face = Face::default();
    let mut recognized = false;
    for field in text.trim_matches('\0').split(['|', '=']) {
        let field = field.trim();
        if let Some(angles) = field.strip_prefix("head#") {
            if let Some(roll) = angles
                .split(',')
                .nth(2)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite())
            {
                face.roll = roll.clamp(-45., 45.).to_radians();
            }
            continue;
        }
        let Some((name, raw)) = field.split_once('&').or_else(|| field.split_once('-')) else {
            continue;
        };
        let Some(v) = raw.trim().parse::<f32>().ok().filter(|v| v.is_finite()) else {
            continue;
        };
        let v = (v / 100.).clamp(0., 1.);
        let name = name.trim();
        let (base, eye) = if let Some(base) = name
            .strip_suffix("_L")
            .or_else(|| name.strip_suffix("Left"))
        {
            (base, 0)
        } else if let Some(base) = name
            .strip_suffix("_R")
            .or_else(|| name.strip_suffix("Right"))
        {
            (base, 1)
        } else {
            if name == "browInnerUp" {
                face.brow[0] += v * 0.5;
                face.brow[1] += v * 0.5;
            }
            continue;
        };
        match base {
            "eyeBlink" => face.blink[eye] = v,
            "eyeWide" => face.wide[eye] = v,
            "eyeSquint" => face.squint[eye] = v,
            // Subject's left eye is displayed on screen right, as in a portrait.
            "eyeLookIn" => face.gaze[eye][0] += if eye == 0 { -v } else { v },
            "eyeLookOut" => face.gaze[eye][0] += if eye == 0 { v } else { -v },
            "eyeLookUp" => face.gaze[eye][1] -= v,
            "eyeLookDown" => face.gaze[eye][1] += v,
            "browOuterUp" => face.brow[eye] += v * 0.5,
            "browDown" => face.brow[eye] -= v,
            _ => continue,
        }
        recognized = true;
    }
    recognized.then_some(face)
}

pub struct Receiver {
    socket: Option<UdpSocket>,
    pub face: Face,
    pub last_frame: Option<Instant>,
    pub packets: u64,
    pub rejected: u64,
    pub peer: Option<SocketAddr>,
    pub status: String,
}

impl Receiver {
    pub fn try_rebind(&mut self, port: u16) -> Result<(), String> {
        if self
            .socket
            .as_ref()
            .and_then(|s| s.local_addr().ok())
            .is_some_and(|a| a.port() == port)
        {
            return Ok(());
        }
        let next = Self::bind(port);
        if next.socket.is_none() {
            return Err(next.status);
        }
        *self = next;
        Ok(())
    }
    pub fn rebind(&mut self, port: u16) {
        self.socket.take(); // release the previous port before binding again
        *self = Self::bind(port);
    }
    pub fn bind(port: u16) -> Self {
        let mut this = Self {
            socket: None,
            face: Face::default(),
            last_frame: None,
            packets: 0,
            rejected: 0,
            peer: None,
            status: String::new(),
        };
        match UdpSocket::bind(("0.0.0.0", port)).and_then(|s| {
            s.set_nonblocking(true)?;
            Ok(s)
        }) {
            Ok(socket) => {
                this.socket = Some(socket);
                this.status = format!("Listening on UDP {port}");
            }
            Err(e) => this.status = format!("Cannot bind UDP {port}: {e}"),
        }
        this
    }
    pub fn connect(&mut self, address: &str) -> Result<(), String> {
        let result = address
            .trim()
            .parse::<IpAddr>()
            .map_err(|e| e.to_string())
            .and_then(|ip| {
                self.socket
                    .as_ref()
                    .ok_or("Listener is unavailable".to_owned())?
                    .send_to(
                        b"iFacialMocap_sahuasouryya9218sauhuiayeta91555dy3719|sendDataVersion=v2",
                        (ip, 49983),
                    )
                    .map_err(|e| e.to_string())
            });
        self.status = match &result {
            Ok(_) => "Start request sent; waiting for face data".into(),
            Err(e) => format!("Connect failed: {e}"),
        };
        result.map(|_| ())
    }
    pub fn poll(&mut self) {
        let Some(socket) = &self.socket else { return };
        let mut buf = [0u8; 16384];
        for _ in 0..128 {
            match socket.recv_from(&mut buf) {
                Ok((len, peer)) => match parse(&buf[..len]) {
                    Some(face) => {
                        self.face = face;
                        self.last_frame = Some(Instant::now());
                        self.peer = Some(peer);
                        self.packets += 1;
                        if self.status != "Receiving face data" {
                            self.status = "Receiving face data".into();
                        }
                    }
                    None => self.rejected += 1,
                },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    self.status = format!("Receive failed: {e}");
                    break;
                }
            }
        }
        if self.status == "Receiving face data" && !self.live() {
            self.status = "Face data timed out".into();
        }
    }
    pub fn live(&self) -> bool {
        self.last_frame
            .is_some_and(|t| t.elapsed() < Duration::from_millis(1000))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_v1_and_negative_head() {
        let f = parse(
            b"eyeBlink_L-100|eyeBlink_R-7|eyeLookOut_L-40|eyeLookUp_L-20|=head#-21,-6,-10,0,0,0|",
        )
        .unwrap();
        assert_eq!(f.blink, [1., 0.07]);
        assert_eq!(f.gaze[0], [0.4, -0.2]);
        assert!((f.roll - (-10f32).to_radians()).abs() < 0.0001);
    }
    #[test]
    fn v2_aliases_clamping_and_nan() {
        let f = parse(b"eyeBlinkLeft&120|eyeWide_R&-20|eyeBlink_R&NaN|eyeLookIn_R&50|").unwrap();
        assert_eq!(f.blink, [1., 0.]);
        assert_eq!(f.wide, [0., 0.]);
        assert_eq!(f.gaze[1][0], 0.5);
        assert!(parse(b"junk|eyeBlink_L-NaN").is_none());
        assert!(parse(&[255, 0]).is_none());
    }
    #[test]
    fn udp_loopback() {
        let mut rx = Receiver::bind(0);
        let addr = rx.socket.as_ref().unwrap().local_addr().unwrap();
        let tx = UdpSocket::bind("127.0.0.1:0").unwrap();
        tx.send_to(b"eyeBlink_L-80|eyeBlink_R-10|", ("127.0.0.1", addr.port()))
            .unwrap();
        for _ in 0..50 {
            rx.poll();
            if rx.live() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(rx.live());
        assert_eq!(rx.face.blink, [0.8, 0.1]);
        assert_eq!(rx.packets, 1);
        assert_eq!(rx.status, "Receiving face data");
        // 不等真实超时，验证状态不会继续沿用连接请求或收包时的旧提示。
        rx.last_frame = Some(Instant::now() - Duration::from_secs(2));
        rx.poll();
        assert!(!rx.live());
        assert_eq!(rx.status, "Face data timed out");
        tx.send_to(b"eyeBlink_L-0|", ("127.0.0.1", addr.port()))
            .unwrap();
        for _ in 0..50 {
            rx.poll();
            if rx.live() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(rx.live());
        assert_eq!(rx.status, "Receiving face data");
    }
}
