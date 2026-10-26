use crate::service::Options;
use drm::{
    Device as _,
    buffer::{Buffer as _, DrmFourcc},
    control::{
        self, AtomicCommitFlags, Device as _, atomic::AtomicModeReq, connector, crtc,
        dumbbuffer::DumbBuffer, framebuffer, plane, property,
    },
};
use eyes::assets::AssetResult;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    os::fd::{AsFd, BorrowedFd},
    path::Path,
};

struct Card(File);
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl control::Device for Card {}
impl Card {
    fn open(path: &Path) -> AssetResult<Self> {
        Ok(Self(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|e| format!("无法打开 DRM 设备 {}：{e}", path.display()))?,
        ))
    }
}

pub fn list(path: &Path) -> AssetResult<()> {
    let card = Card::open(path)?;
    let res = card.resource_handles()?;
    println!("设备 {}，CRTC：{:?}", path.display(), res.crtcs());
    for handle in res.connectors() {
        let c = card.get_connector(*handle, false)?;
        println!("{}，ID {}，{:?}", c, u32::from(*handle), c.state());
        for mode in c.modes() {
            println!(
                "  {}x{}@{}{}",
                mode.size().0,
                mode.size().1,
                mode.vrefresh(),
                if mode.mode_type().contains(control::ModeTypeFlags::PREFERRED) {
                    " 首选"
                } else {
                    ""
                }
            );
        }
    }
    Ok(())
}

struct Scanout {
    buffer: DumbBuffer,
    fb: framebuffer::Handle,
}
struct Display {
    eye: usize,
    description: String,
    connector: connector::Info,
    crtc: crtc::Handle,
    plane: plane::Handle,
    mode: control::Mode,
    frames: Vec<Scanout>,
    front: usize,
    fb_property: property::Handle,
}
struct Group {
    card: Card,
    displays: Vec<Display>,
    originals: Vec<(crtc::Info, Vec<connector::Handle>)>,
    started: bool,
}

fn choose_mode(c: &connector::Info, requested: Option<&str>) -> AssetResult<control::Mode> {
    if let Some(requested) = requested {
        for mode in c.modes() {
            let size = format!("{}x{}", mode.size().0, mode.size().1);
            if requested == size || requested == format!("{size}@{}", mode.vrefresh()) {
                return Ok(*mode);
            }
        }
        return Err(format!("连接器 {c} 没有模式 {requested}，请先使用 --list-drm").into());
    }
    c.modes()
        .iter()
        .find(|m| m.mode_type().contains(control::ModeTypeFlags::PREFERRED))
        .or_else(|| c.modes().first())
        .copied()
        .ok_or_else(|| format!("连接器 {c} 没有可用显示模式").into())
}

impl Group {
    fn new(path: &Path, requests: &[(usize, Option<&str>, Option<&str>)]) -> AssetResult<Self> {
        let card = Card::open(path)?;
        card.acquire_master_lock().map_err(|e| {
            format!("无法取得 DRM master（请在独立终端停用占用该设备的桌面合成器）：{e}")
        })?;
        card.set_client_capability(drm::ClientCapability::UniversalPlanes, true)?;
        card.set_client_capability(drm::ClientCapability::Atomic, true)?;
        let res = card.resource_handles()?;
        let connections: Vec<_> = res
            .connectors()
            .iter()
            .map(|c| card.get_connector(*c, true))
            .collect::<Result<_, _>>()?;
        let mut chosen = Vec::new();
        for &(eye, requested, mode) in requests {
            let c = connections
                .iter()
                .find(|c| {
                    c.state() == connector::State::Connected
                        && !chosen.iter().any(
                            |(_, selected, _): &(usize, connector::Info, control::Mode)| {
                                selected.handle() == c.handle()
                            },
                        )
                        && requested.is_none_or(|name| {
                            name == c.to_string() || name == u32::from(c.handle()).to_string()
                        })
                })
                .ok_or_else(|| {
                    format!(
                        "{} 眼连接器 {} 未连接、被重复选择或不存在",
                        if eye == 0 { "左" } else { "右" },
                        requested.unwrap_or("自动")
                    )
                })?;
            chosen.push((eye, c.clone(), choose_mode(c, mode)?));
        }
        let planes = card.plane_handles()?;
        let mut candidates = Vec::new();
        for (_, c, _) in &chosen {
            let mut options = Vec::new();
            for encoder in c.encoders() {
                let encoder = card.get_encoder(*encoder)?;
                for crtc in res.filter_crtcs(encoder.possible_crtcs()) {
                    for &plane in &planes {
                        let info = card.get_plane(plane)?;
                        if !res.filter_crtcs(info.possible_crtcs()).contains(&crtc)
                            || !info.formats().contains(&(DrmFourcc::Xrgb8888 as u32))
                        {
                            continue;
                        }
                        let props = card.get_properties(plane)?;
                        let primary = props.iter().any(|(&id, &value)| {
                            card.get_property(id).is_ok_and(|p| {
                                p.name().to_bytes() == b"type"
                                    && value == control::PlaneType::Primary as u64
                            })
                        });
                        if primary && !options.contains(&(crtc, plane)) {
                            options.push((crtc, plane));
                        }
                    }
                }
            }
            candidates.push(options);
        }
        // 搜索整个双眼组合，避免先分配的眼睛占用另一只眼睛唯一可用的 CRTC。
        let allocation = crate::service::allocation(&candidates)
            .ok_or("无法为两只眼睛分配独立 CRTC 和 XRGB8888 主平面；请检查硬件显示管线")?;
        let mut group = Self {
            card,
            displays: Vec::new(),
            originals: Vec::new(),
            started: false,
        };
        for ((eye, c, mode), (crtc, plane)) in chosen.into_iter().zip(allocation) {
            let old = group.card.get_crtc(crtc)?;
            let old_connectors = connections
                .iter()
                .filter(|c| {
                    c.current_encoder()
                        .and_then(|e| group.card.get_encoder(e).ok())
                        .and_then(|e| e.crtc())
                        == Some(crtc)
                })
                .map(|c| c.handle())
                .collect();
            group.originals.push((old, old_connectors));
            let properties = group.card.get_properties(plane)?.as_hashmap(&group.card)?;
            let fb_property = properties
                .get("FB_ID")
                .ok_or("主平面缺少 FB_ID 属性")?
                .handle();
            let display = Display {
                eye,
                description: format!(
                    "{} / {} / {}x{}@{}",
                    path.display(),
                    c,
                    mode.size().0,
                    mode.size().1,
                    mode.vrefresh()
                ),
                connector: c,
                crtc,
                plane,
                mode,
                frames: Vec::new(),
                front: 0,
                fb_property,
            };
            group.displays.push(display);
            let d = group.displays.last_mut().unwrap();
            for _ in 0..2 {
                let mut buffer = group.card.create_dumb_buffer(
                    (mode.size().0.into(), mode.size().1.into()),
                    DrmFourcc::Xrgb8888,
                    32,
                )?;
                let fb = match group.card.add_framebuffer(&buffer, 24, 32) {
                    Ok(fb) => fb,
                    Err(e) => {
                        let _ = group.card.destroy_dumb_buffer(buffer);
                        return Err(e.into());
                    }
                };
                d.frames.push(Scanout { buffer, fb });
                group.card.map_dumb_buffer(&mut buffer)?.fill(0);
            }
        }
        // 在改动硬件前检查整组配置；首帧真正渲染完再开启输出，避免显示未初始化像素。
        group.commit_initial(AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET)?;
        Ok(group)
    }

    fn commit_initial(&self, flags: AtomicCommitFlags) -> AssetResult<()> {
        let mut request = AtomicModeReq::new();
        let mut blobs = Vec::new();
        let result = (|| -> AssetResult<()> {
            for d in &self.displays {
                let cp = self
                    .card
                    .get_properties(d.connector.handle())?
                    .as_hashmap(&self.card)?;
                let rp = self.card.get_properties(d.crtc)?.as_hashmap(&self.card)?;
                let pp = self.card.get_properties(d.plane)?.as_hashmap(&self.card)?;
                let blob = self.card.create_property_blob(&d.mode)?;
                blobs.push(u64::from(blob));
                add(
                    &mut request,
                    d.connector.handle(),
                    &cp,
                    "CRTC_ID",
                    property::Value::CRTC(Some(d.crtc)),
                )?;
                add(&mut request, d.crtc, &rp, "MODE_ID", blob)?;
                add(
                    &mut request,
                    d.crtc,
                    &rp,
                    "ACTIVE",
                    property::Value::Boolean(true),
                )?;
                add(
                    &mut request,
                    d.plane,
                    &pp,
                    "FB_ID",
                    property::Value::Framebuffer(Some(d.frames[1 - d.front].fb)),
                )?;
                add(
                    &mut request,
                    d.plane,
                    &pp,
                    "CRTC_ID",
                    property::Value::CRTC(Some(d.crtc)),
                )?;
                for (name, value) in [
                    ("SRC_X", 0),
                    ("SRC_Y", 0),
                    ("SRC_W", u64::from(d.mode.size().0) << 16),
                    ("SRC_H", u64::from(d.mode.size().1) << 16),
                    ("CRTC_X", 0),
                    ("CRTC_Y", 0),
                    ("CRTC_W", u64::from(d.mode.size().0)),
                    ("CRTC_H", u64::from(d.mode.size().1)),
                ] {
                    add(
                        &mut request,
                        d.plane,
                        &pp,
                        name,
                        property::Value::UnsignedRange(value),
                    )?;
                }
            }
            self.card.atomic_commit(flags, request)?;
            Ok(())
        })();
        for blob in blobs {
            let _ = self.card.destroy_property_blob(blob);
        }
        result
    }

    fn present(&mut self) -> AssetResult<()> {
        if !self.started {
            self.commit_initial(AtomicCommitFlags::ALLOW_MODESET)?;
            self.started = true;
        } else {
            let mut request = AtomicModeReq::new();
            for d in &self.displays {
                request.add_property(
                    d.plane,
                    d.fb_property,
                    property::Value::Framebuffer(Some(d.frames[1 - d.front].fb)),
                );
            }
            // 阻塞原子翻页完成后才复用旧缓冲，同卡双眼在一次提交中切换。
            self.card
                .atomic_commit(AtomicCommitFlags::empty(), request)?;
        }
        for d in &mut self.displays {
            d.front = 1 - d.front;
        }
        Ok(())
    }
}

fn add<H: control::ResourceHandle>(
    request: &mut AtomicModeReq,
    object: H,
    properties: &HashMap<String, property::Info>,
    name: &str,
    value: property::Value<'_>,
) -> AssetResult<()> {
    let id = properties
        .get(name)
        .ok_or_else(|| format!("DRM 缺少属性 {name}"))?
        .handle();
    request.add_property(object, id, value);
    Ok(())
}

impl Drop for Group {
    fn drop(&mut self) {
        if self.started {
            for (old, connectors) in &self.originals {
                if let Err(e) = self.card.set_crtc(
                    old.handle(),
                    old.framebuffer(),
                    old.position(),
                    connectors,
                    old.mode(),
                ) {
                    eprintln!("恢复原 DRM 显示状态失败：{e}");
                }
            }
        }
        for d in &self.displays {
            for frame in &d.frames {
                let _ = self.card.destroy_framebuffer(frame.fb);
                let _ = self.card.destroy_dumb_buffer(frame.buffer);
            }
        }
        let _ = self.card.release_master_lock();
    }
}

pub struct Outputs {
    groups: Vec<Group>,
}
impl Outputs {
    pub fn new(options: &Options) -> AssetResult<Self> {
        let right = options.right_device.as_deref().unwrap_or(&options.device);
        let same = std::fs::canonicalize(&options.device)? == std::fs::canonicalize(right)?;
        let request = |i: usize| {
            (
                i,
                options.connectors[i].as_deref(),
                options.modes[i].as_deref(),
            )
        };
        let groups = if same {
            vec![Group::new(&options.device, &[request(0), request(1)])?]
        } else {
            vec![
                Group::new(&options.device, &[request(0)])?,
                Group::new(right, &[request(1)])?,
            ]
        };
        let result = Self { groups };
        for description in result.descriptions() {
            eprintln!("DRM 输出：{description}");
        }
        Ok(result)
    }
    pub fn sizes(&self) -> [(u32, u32); 2] {
        let mut sizes = [(0, 0); 2];
        for g in &self.groups {
            for d in &g.displays {
                sizes[d.eye] = (d.mode.size().0.into(), d.mode.size().1.into());
            }
        }
        sizes
    }
    pub fn descriptions(&self) -> [String; 2] {
        let mut names = [String::new(), String::new()];
        for g in &self.groups {
            for d in &g.displays {
                names[d.eye] = d.description.clone();
            }
        }
        names
    }
    pub fn write(&mut self, eye: usize, pixels: &[u8]) -> AssetResult<()> {
        for g in &mut self.groups {
            if let Some(d) = g.displays.iter_mut().find(|d| d.eye == eye) {
                let frame = &mut d.frames[1 - d.front];
                let stride = frame.buffer.pitch() as usize;
                let width = d.mode.size().0 as usize * 4;
                if pixels.len() != width * d.mode.size().1 as usize {
                    return Err("DRM 像素尺寸不匹配".into());
                }
                let mut map = g.card.map_dumb_buffer(&mut frame.buffer)?;
                for (src, dst) in pixels.chunks_exact(width).zip(map.chunks_exact_mut(stride)) {
                    dst[..width].copy_from_slice(src);
                }
                return Ok(());
            }
        }
        Err("DRM 眼睛输出不存在".into())
    }
    pub fn present(&mut self) -> AssetResult<()> {
        for g in &mut self.groups {
            g.present()?;
        }
        Ok(())
    }
}
