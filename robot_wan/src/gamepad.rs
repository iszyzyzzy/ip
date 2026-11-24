use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

use serde_json::{Value, json};

use crate::rpc::{ApiError, Robot};

#[derive(Default)]
struct Status {
    device: String,
    error: String,
    controlling: bool,
    held: Vec<&'static str>,
    bindings: BTreeMap<String, String>,
}

pub struct Gamepad {
    path: PathBuf,
    status: Mutex<Status>,
}

fn chord_exists(chord: &str) -> bool {
    matches!(
        chord.split_once('+'),
        Some(("RB" | "L3" | "R3", "A" | "B" | "X" | "Y"))
    )
}

impl Gamepad {
    pub fn load() -> Self {
        let dir = std::env::var_os("STATE_DIRECTORY")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
                home.map(PathBuf::from)
                    .unwrap_or_default()
                    .join(".config/robot-remote")
            });
        let path = dir.join("gamepad.json");
        let mut status = Status::default();
        match std::fs::read(&path) {
            Ok(data) => match serde_json::from_slice(&data) {
                Ok(bindings) => status.bindings = bindings,
                Err(e) => status.error = format!("读取手柄绑定失败：{e}"),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => status.error = format!("读取手柄绑定失败：{e}"),
        }
        Self {
            path,
            status: Mutex::new(status),
        }
    }

    pub fn status(&self) -> Value {
        let s = self.status.lock().unwrap();
        json!({"device":s.device,"error":s.error,"controlling":s.controlling,
            "held":s.held,"bindings":s.bindings})
    }

    pub async fn set_binding(&self, robot: &Robot, params: &Value) -> Result<Value, ApiError> {
        let chord = params["chord"].as_str().filter(|s| chord_exists(s));
        let (Some(chord), Some(action)) = (chord, params["action"].as_str()) else {
            return Err(ApiError::new("INVALID_ARGUMENT", "请选择组合键和动作"));
        };
        if !action.is_empty() {
            let library = robot.call("list_actions", json!({})).await?;
            if !library["actions"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|row| row["deleted"] == false && row["action"]["name"] == action)
            {
                return Err(ApiError::new("NOT_FOUND", "动作已移除，请重新选择"));
            }
        }
        let mut s = self.status.lock().unwrap();
        let mut bindings = s.bindings.clone();
        if action.is_empty() {
            bindings.remove(chord);
        } else {
            bindings.insert(chord.into(), action.into());
        }
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(self.path.parent().unwrap())?;
            let tmp = self.path.with_extension("tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(&bindings)?)?;
            std::fs::rename(tmp, &self.path)
        };
        write().map_err(|e| ApiError::new("INTERNAL", &format!("保存手柄绑定失败：{e}")))?;
        s.bindings = bindings;
        Ok(json!({"bindings":s.bindings}))
    }

    pub async fn run(&self, robot: Robot) {
        #[cfg(target_os = "linux")]
        linux::run(self, robot).await;
        #[cfg(not(target_os = "linux"))]
        let _ = robot;
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        collections::BTreeSet,
        fs::{File, OpenOptions},
        io::{self, Read},
        os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
        path::Path,
        time::{Duration, Instant},
    };

    use serde_json::{Value, json};

    use super::Gamepad;
    use crate::rpc::Robot;

    fn button(code: u16) -> Option<&'static str> {
        Some(match code {
            0x130 => "A",
            0x131 => "B",
            0x133 => "X",
            0x134 => "Y",
            0x136 => "LB",
            0x137 => "RB",
            0x13a => "View",
            0x13b => "Menu",
            0x13d => "L3",
            0x13e => "R3",
            _ => return None,
        })
    }

    struct Joystick {
        file: File,
        axes: Vec<u8>,
        buttons: Vec<u16>,
        name: String,
        held: BTreeSet<&'static str>,
        x: f64,
        y: f64,
    }

    type Press = (&'static str, BTreeSet<&'static str>);

    impl Joystick {
        fn open(path: &Path) -> io::Result<Self> {
            let sys = Path::new("/sys/class/input")
                .join(path.file_name().unwrap())
                .join("device");
            let address = std::env::var("ROBOT_REMOTE_GAMEPAD_MAC").unwrap_or_default();
            if std::fs::read_to_string(sys.join("id/bustype"))?.trim() != "0005"
                || (!address.is_empty()
                    && !std::fs::read_to_string(sys.join("uniq"))?
                        .trim()
                        .eq_ignore_ascii_case(address.trim()))
            {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "手柄地址或总线不匹配",
                ));
            }
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path)?;
            // joydev 的 ioctl 编码带输出缓冲区尺寸，键序因手柄型号而异。
            let read_map = |code: u32, buffer: &mut [u8]| {
                let request = (0x8000_6a00 | ((buffer.len() as u32) << 16) | code) as libc::c_ulong;
                let result = unsafe { libc::ioctl(file.as_raw_fd(), request, buffer.as_mut_ptr()) };
                if result < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            };
            let mut axes = vec![0; 64];
            let mut buttons = [0; 1024];
            let mut n = [0; 1];
            let mut name = [0; 128];
            read_map(0x11, &mut n)?;
            read_map(0x32, &mut axes)?;
            axes.truncate(n[0].into());
            read_map(0x12, &mut n)?;
            read_map(0x34, &mut buttons)?;
            let buttons: Vec<_> = buttons
                .chunks_exact(2)
                .take(n[0].into())
                .map(|p| u16::from_ne_bytes([p[0], p[1]]))
                .collect();
            if ![0, 1].iter().all(|a| axes.contains(a))
                || buttons
                    .iter()
                    .filter_map(|b| button(*b))
                    .collect::<BTreeSet<_>>()
                    .len()
                    != 10
            {
                return Err(io::Error::new(io::ErrorKind::Unsupported, "手柄键位不匹配"));
            }
            read_map(0x13, &mut name)?;
            let name =
                String::from_utf8_lossy(name.split(|b| *b == 0).next().unwrap()).into_owned();
            Ok(Self {
                file,
                axes,
                buttons,
                name,
                held: BTreeSet::new(),
                x: 0.,
                y: 0.,
            })
        }

        fn read(&mut self) -> io::Result<Vec<Press>> {
            let mut presses = Vec::new();
            let mut event = [0; 8];
            loop {
                match self.file.read(&mut event) {
                    Ok(8) => {}
                    Ok(_) => return Err(io::ErrorKind::UnexpectedEof.into()),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(presses),
                    Err(e) => return Err(e),
                }
                let value = i16::from_ne_bytes([event[4], event[5]]);
                let index = event[7] as usize;
                if event[6] & 0x7f == 1
                    && let Some(key) = self.buttons.get(index).and_then(|b| button(*b))
                {
                    if value == 0 {
                        self.held.remove(key);
                    } else if self.held.insert(key) && event[6] & 0x80 == 0 {
                        presses.push((key, self.held.clone()));
                    }
                } else if event[6] & 0x7f == 2 {
                    let v = (f64::from(value) / 32767.).max(-1.);
                    let v = if v.abs() < 0.12 { 0. } else { v };
                    match self.axes.get(index) {
                        Some(0) => self.x = v,
                        Some(1) => self.y = v,
                        _ => {}
                    }
                }
            }
        }
    }

    #[derive(Default)]
    struct Local {
        token: String,
        renewed: Option<Instant>,
        chassis: Value,
        driving: bool,
    }

    impl Local {
        async fn release(&mut self, robot: &Robot) {
            if !self.token.is_empty() {
                let _ = robot
                    .call("release", json!({"token":std::mem::take(&mut self.token)}))
                    .await;
            }
            self.driving = false;
        }

        async fn tick(
            &mut self,
            robot: &Robot,
            gamepad: &Gamepad,
            pad: &Joystick,
            presses: &[Press],
        ) -> Result<(), tonic::Status> {
            let pressed = |key| presses.iter().any(|(button, _)| *button == key);
            if pressed("View") {
                robot.call("stop", json!({"latch":true})).await?;
                self.token.clear();
                self.driving = false;
                return Ok(());
            }
            if pressed("Menu") {
                if !self.token.is_empty() {
                    self.release(robot).await;
                } else if pad.x == 0.
                    && pad.y == 0.
                    && pad.held.iter().all(|button| *button == "Menu")
                {
                    self.chassis =
                        robot.call("get_config", json!({})).await?["config"]["chassis"].clone();
                    let lease = robot
                        .call("acquire", json!({"owner":"蓝牙手柄","ttl_ms":1000}))
                        .await?;
                    self.token = lease["token"].as_str().unwrap_or_default().into();
                    self.renewed = Some(Instant::now());
                }
                return Ok(());
            }
            if self.token.is_empty() {
                return Ok(());
            }
            if self
                .renewed
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(250))
            {
                robot.call("renew", json!({"token":self.token})).await?;
                self.renewed = Some(Instant::now());
            }
            if pad.held.contains("LB")
                && (self.driving || pressed("LB"))
                && self.chassis["enabled"] == true
            {
                let speed = -pad.y
                    * self.chassis["max_speed_m_s"]
                        .as_f64()
                        .unwrap_or_default()
                        .min(0.3);
                let steer = -pad.x
                    * self.chassis["max_steering_rad"]
                        .as_f64()
                        .unwrap_or_default()
                        .min(0.698);
                robot
                    .call(
                        "drive",
                        json!({"token":self.token,"targets":{"chassis":{
                    "mode":"ACKERMANN","speed_m_s":speed,"steering_rad":steer,"ttl_ms":200}}}),
                    )
                    .await?;
                self.driving = true;
            } else if self.driving {
                self.driving = false;
                robot.call("drive", json!({"token":self.token,"targets":{"chassis":{"mode":"PARK","ttl_ms":200}}})).await?;
            } else if !pad.held.contains("LB") {
                let name = {
                    let status = gamepad.status.lock().unwrap();
                    presses
                        .iter()
                        .filter(|(_, held)| held.len() == 2)
                        .find_map(|(k, held)| {
                            let modifier = held.iter().find(|&&button| button != *k)?;
                            status.bindings.get(&format!("{modifier}+{k}")).cloned()
                        })
                };
                if let Some(name) = name {
                    robot
                        .call("run_action", json!({"token":self.token,"name":name}))
                        .await?;
                }
            }
            Ok(())
        }
    }

    pub async fn run(gamepad: &Gamepad, robot: Robot) {
        loop {
            let paths = std::fs::read_dir("/dev/input")
                .into_iter()
                .flatten()
                .flatten();
            let mut paths: Vec<_> = paths
                .map(|entry| entry.path())
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("js"))
                })
                .collect();
            paths.sort();
            if let Some(mut pad) = paths.iter().find_map(|path| Joystick::open(path).ok()) {
                let mut local = Local::default();
                gamepad.status.lock().unwrap().device = pad.name.clone();
                while let Ok(presses) = pad.read() {
                    let error = local.tick(&robot, gamepad, &pad, &presses).await.err();
                    if error.is_some() {
                        local.release(&robot).await;
                    }
                    {
                        let mut status = gamepad.status.lock().unwrap();
                        status.controlling = !local.token.is_empty();
                        status.held = pad.held.iter().copied().collect();
                        if let Some(e) = error {
                            status.error = e.message().into();
                        } else if !presses.is_empty() {
                            status.error.clear();
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                local.release(&robot).await;
                let mut status = gamepad.status.lock().unwrap();
                status.device.clear();
                status.held.clear();
                status.controlling = false;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
