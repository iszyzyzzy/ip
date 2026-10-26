use crate::{EyesApp, assets, offscreen};
use eyes::assets::AssetResult;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Output {
    Window,
    ControlOnly,
    Drm,
    Offscreen,
}

pub struct Options {
    pub output: Output,
    pub settings: PathBuf,
    pub screenshot: Option<PathBuf>,
    pub exit_after: Option<f32>,
    pub socket: Option<PathBuf>,
    pub assets: PathBuf,
    pub device: PathBuf,
    pub right_device: Option<PathBuf>,
    pub connectors: [Option<String>; 2],
    pub modes: [Option<String>; 2],
    pub fps: u32,
    pub size: (u32, u32),
    pub preview: Option<PathBuf>,
    pub list: bool,
    pub help: bool,
}

impl Options {
    pub fn parse(args: impl Iterator<Item = String>) -> AssetResult<Self> {
        let mut o = Self {
            output: Output::Window,
            settings: "eyes-settings.json".into(),
            screenshot: None,
            exit_after: None,
            socket: Some(std::env::temp_dir().join("oculus-eyes.sock")),
            assets: assets::find_root(),
            device: "/dev/dri/card0".into(),
            right_device: None,
            connectors: [None, None],
            modes: [None, None],
            fps: 60,
            size: (480, 480),
            preview: None,
            list: false,
            help: false,
        };
        let mut args = args;
        let mut selected = false;
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} 缺少参数"));
            match arg.as_str() {
                "--headless" | "--drm" | "--offscreen" | "--control-only" => {
                    // 无头部署仍需显示眼睛，两种名称指向同一个 DRM 渲染后端。
                    let output = match arg.as_str() {
                        "--headless" | "--drm" => Output::Drm,
                        "--control-only" => Output::ControlOnly,
                        _ => Output::Offscreen,
                    };
                    if selected && o.output != output {
                        return Err("只能选择一种输出方式".into());
                    }
                    selected = true;
                    o.output = output;
                    if o.output == Output::Offscreen {
                        o.preview = Some(value()?.into());
                    }
                }
                "--assets" => o.assets = value()?.into(),
                "--socket" => o.socket = Some(value()?.into()),
                "--no-grpc" => o.socket = None,
                "--settings" => o.settings = value()?.into(),
                "--screenshot" => o.screenshot = Some(value()?.into()),
                "--exit-after" => {
                    let t = value()?.parse::<f32>()?;
                    if !t.is_finite() || t <= 0. {
                        return Err("退出时间必须是正数".into());
                    }
                    o.exit_after = Some(t);
                }
                "--drm-device" => o.device = value()?.into(),
                "--right-drm-device" => o.right_device = Some(value()?.into()),
                "--left-connector" => o.connectors[0] = Some(value()?),
                "--right-connector" => o.connectors[1] = Some(value()?),
                "--left-mode" => o.modes[0] = Some(value()?),
                "--right-mode" => o.modes[1] = Some(value()?),
                "--fps" => {
                    o.fps = value()?.parse()?;
                    if !(1..=120).contains(&o.fps) {
                        return Err("帧率范围为 1–120".into());
                    }
                }
                "--size" => {
                    let v = value()?;
                    let (w, h) = v.split_once('x').ok_or("尺寸格式为 480x480")?;
                    o.size = (w.parse()?, h.parse()?);
                    if o.size.0 == 0 || o.size.1 == 0 || o.size.0 > 4096 || o.size.1 > 4096 {
                        return Err("离屏尺寸范围为 1–4096".into());
                    }
                }
                "--list-drm" => o.list = true,
                "--help" | "-h" => o.help = true,
                _ => return Err(format!("未知参数：{arg}").into()),
            }
        }
        if o.output != Output::Window && o.screenshot.is_some() {
            return Err("--screenshot 用于窗口；双眼离屏截图请使用 --offscreen DIR".into());
        }
        Ok(o)
    }
}

pub fn help() {
    println!(
        "眼屏应用\n默认：Windows / Linux 窗口渲染\n  --headless / --drm              Linux 无桌面 GPU 渲染，DRM/KMS 直连双眼屏幕\n  --control-only                 仅控制诊断，不创建 GPU，不输出图像\n  --drm-device /dev/dri/card0      左眼设备，也作为默认右眼设备\n  --right-drm-device PATH         可选的另一块 DRM 设备\n  --left-connector DSI-1          左眼连接器名称或 ID\n  --right-connector DSI-2         右眼连接器名称或 ID\n  --left-mode WxH[@Hz]            左眼模式，默认首选模式\n  --right-mode WxH[@Hz]           右眼模式，默认首选模式\n  --list-drm                      列出设备的连接器、模式和 CRTC\n  --offscreen DIR --size 480x480  无窗口 GPU 渲染左右眼 PNG，便于检查\n  --fps 60                        无窗口循环帧率\n  --assets DIR --settings FILE --socket PATH --no-grpc\n  --exit-after SECONDS --screenshot FILE.ppm（窗口）\n所有后端保留 gRPC 和 UDP 面捕；--no-grpc 仅关闭 gRPC\nF5 重载素材 | Tab 面板 | F11 全屏 | Space 眨眼"
    );
}

pub fn run(o: Options) -> AssetResult<()> {
    #[cfg(target_os = "linux")]
    let mut displays = if o.output == Output::Drm {
        Some(crate::drm_output::Outputs::new(&o)?)
    } else {
        None
    };
    #[cfg(not(target_os = "linux"))]
    if o.output == Output::Drm {
        return Err("DRM/KMS 输出需要 Linux；本机可用 --offscreen 检查双眼渲染".into());
    }
    let mut sizes = [o.size; 2];
    #[cfg(target_os = "linux")]
    if let Some(d) = &displays {
        sizes = d.sizes();
    }
    #[cfg(not(target_os = "linux"))]
    let _ = &mut sizes;
    let (renderer, art) = if o.output == Output::ControlOnly {
        (None, assets::Pack::load(&o.assets)?.art)
    } else {
        let (renderer, art) = offscreen::Renderer::new(&o.assets, sizes)?;
        (Some(renderer), art)
    };
    let mut app = EyesApp::without_window(
        o.settings,
        None,
        o.exit_after,
        o.socket,
        o.assets,
        art,
        None,
        renderer,
    )?;
    app.settings.show_controls = false;
    app.output_mode = match o.output {
        Output::ControlOnly => "control_only",
        Output::Drm => "drm",
        _ => "offscreen",
    }
    .into();
    #[cfg(target_os = "linux")]
    if let Some(d) = &displays {
        app.outputs = d.descriptions();
    }
    if let Some(dir) = &o.preview {
        std::fs::create_dir_all(dir)?;
    }
    eprintln!("输出方式：{}；gRPC：{}", app.output_mode, app.rpc_status);
    let ctx = eframe::egui::Context::default();
    let period = Duration::from_secs_f64(1. / o.fps as f64);
    let stopping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Some(hub) = &app.control {
        hub.watch_interrupt(stopping.clone());
    }
    #[cfg(target_os = "linux")]
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, stopping.clone())?;
    }
    if let Some(hub) = &app.control {
        *hub.state.lock().unwrap() = app.state();
    }
    let mut captured = false;
    while !stopping.load(std::sync::atomic::Ordering::Relaxed) {
        let now = Instant::now();
        let t = app.start.elapsed().as_secs_f32();
        let dt = now.duration_since(app.last).as_secs_f32().clamp(0.001, 0.1);
        app.last = now;
        app.receiver.poll();
        let replies = app.commands(&ctx, t);
        app.advance(t, dt, [0.; 2]);
        if let Some(hub) = &app.control {
            *hub.state.lock().unwrap() = app.state();
        }
        for (reply, revision) in replies {
            let _ = reply.send(Ok(revision));
        }
        if let Some(renderer) = &app.offscreen {
            let parameters = app.uniforms([1., 1.], t);
            for (eye, size) in sizes.into_iter().enumerate() {
                let mut pixels = renderer.render(eye, parameters)?;
                #[cfg(target_os = "linux")]
                if let Some(d) = &mut displays {
                    d.write(eye, &pixels)?;
                }
                if !captured
                    && t >= 0.8
                    && let Some(dir) = &o.preview
                {
                    for pixel in pixels.as_chunks_mut::<4>().0 {
                        pixel.swap(0, 2);
                    }
                    image::save_buffer(
                        dir.join(if eye == 0 { "left.png" } else { "right.png" }),
                        &pixels,
                        size.0,
                        size.1,
                        image::ColorType::Rgba8,
                    )?;
                }
            }
            #[cfg(target_os = "linux")]
            if let Some(d) = &mut displays {
                d.present()?;
            }
            if t >= 0.8 {
                captured = true;
            }
        }
        if app.exit_after.is_some_and(|limit| t >= limit) {
            break;
        }
        std::thread::sleep(period.saturating_sub(now.elapsed()));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn allocation<A: Copy + PartialEq, B: Copy + PartialEq>(
    candidates: &[Vec<(A, B)>],
) -> Option<Vec<(A, B)>> {
    fn visit<A: Copy + PartialEq, B: Copy + PartialEq>(
        c: &[Vec<(A, B)>],
        picked: &mut Vec<(A, B)>,
    ) -> bool {
        if picked.len() == c.len() {
            return true;
        }
        for &(a, b) in &c[picked.len()] {
            if picked.iter().any(|&(x, y)| a == x || b == y) {
                continue;
            }
            picked.push((a, b));
            if visit(c, picked) {
                return true;
            }
            picked.pop();
        }
        false
    }
    let mut picked = Vec::new();
    visit(candidates, &mut picked).then_some(picked)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_avoids_greedy_conflict() {
        assert_eq!(
            super::allocation(&[vec![(1, 10), (2, 20)], vec![(1, 10)]]),
            Some(vec![(2, 20), (1, 10)])
        );
        assert!(super::allocation(&[vec![(1, 10)], vec![(1, 20)]]).is_none());
    }
    #[test]
    fn startup_options_reject_invalid_combinations() {
        for args in [
            vec!["--headless", "--control-only"],
            vec!["--headless", "--offscreen", "preview"],
            vec!["--fps", "0"],
            vec!["--size", "0x480"],
            vec!["--exit-after", "NaN"],
        ] {
            assert!(Options::parse(args.into_iter().map(String::from)).is_err());
        }
        assert_eq!(
            Options::parse(["--headless".into()].into_iter())
                .unwrap()
                .output,
            Output::Drm
        );
        assert_eq!(
            Options::parse(["--headless".into(), "--drm".into()].into_iter())
                .unwrap()
                .output,
            Output::Drm
        );
        assert_eq!(
            Options::parse(["--control-only".into()].into_iter())
                .unwrap()
                .output,
            Output::ControlOnly
        );
        assert!(
            Options::parse(["--headless".into(), "--no-grpc".into()].into_iter())
                .unwrap()
                .socket
                .is_none()
        );
    }
}
