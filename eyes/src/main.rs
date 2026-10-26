#![recursion_limit = "256"]
#[cfg(target_os = "linux")]
mod drm_output;
mod offscreen;
mod render;
mod service;
mod tracking;

use eframe::egui::{self, Color32, RichText};
use eyes::assets::{self, Art};
use eyes::control::{Command, ControlHub, pb};
use eyes::motion::{Blinker, Jelly, Layers, Shape, Simulator, Spring};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Instant};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
enum Mode {
    Idle,
    Mouse,
    Tracking,
    External,
    Simulator,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    mode: Mode,
    port: u16,
    phone: String,
    scale: f32,
    separation: f32,
    iris_size: f32,
    pupil: f32,
    tilt: f32,
    glow: f32,
    smoothing: f32,
    iris: [f32; 3],
    background: [f32; 3],
    expression: f32,
    auto_blink: bool,
    mirror: bool,
    head_roll: bool,
    jelly: Jelly,
    layers: Layers,
    simulator: Simulator,
    shape: Shape,
    blink_interval: f32,
    blink_duration: f32,
    blink_variation: f32,
    show_controls: bool,
    fullscreen: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Idle,
            port: 49983,
            phone: String::new(),
            scale: 1.,
            separation: 0.48,
            iris_size: 0.218,
            pupil: 0.36,
            tilt: 0.045,
            glow: 0.,
            smoothing: 14.,
            iris: [1., 1., 1.],
            background: [1., 0.945, 0.94],
            expression: 0.,
            auto_blink: true,
            mirror: false,
            head_roll: true,
            jelly: Jelly::default(),
            layers: Layers::default(),
            simulator: Simulator::default(),
            shape: Shape::default(),
            blink_interval: 3.5,
            blink_duration: 0.24,
            blink_variation: 0.3,
            show_controls: true,
            fullscreen: false,
        }
    }
}
impl Settings {
    fn rpc(&self) -> pb::Config {
        let s = self;
        let color = |rgb: [f32; 3]| {
            Some(pb::Color {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
            })
        };
        pb::Config {
            mode: Some(match s.mode {
                Mode::Idle => pb::Mode::Idle,
                Mode::Mouse => pb::Mode::Mouse,
                Mode::Tracking => pb::Mode::Ifacialmocap,
                Mode::External => pb::Mode::External,
                Mode::Simulator => pb::Mode::Simulator,
            } as i32),
            scale: Some(s.scale),
            spacing: Some(s.separation),
            iris_size: Some(s.iris_size),
            pupil: Some(s.pupil),
            tilt: Some(s.tilt),
            glow: Some(s.glow),
            expression: Some(s.expression),
            response: Some(s.smoothing),
            iris: color(s.iris),
            background: color(s.background),
            auto_blink: Some(s.auto_blink),
            mirror_tracking: Some(s.mirror),
            follow_head_roll: Some(s.head_roll),
            show_controls: Some(s.show_controls),
            fullscreen: Some(s.fullscreen),
            jelly: Some(s.jelly.proto()),
            layers: Some(s.layers.proto()),
            simulator: Some(s.simulator.proto()),
            blink_interval: Some(s.blink_interval),
            blink_duration: Some(s.blink_duration),
            blink_variation: Some(s.blink_variation),
            tracking_port: Some(s.port as u32),
            phone_ip: Some(s.phone.clone()),
            art: Some(s.shape.proto()),
        }
    }

    fn apply_art(&mut self, art: &Art) {
        self.shape = Shape::from_art(art);
        let d = &art.defaults;
        self.iris_size = d.iris_size;
        self.pupil = d.pupil;
        self.tilt = d.tilt;
        self.glow = d.glow;
        self.iris = d.iris;
        self.background = d.background;
    }
    fn sanitize(&mut self) {
        self.scale = self.scale.clamp(0.5, 1.5);
        self.separation = self.separation.clamp(0.38, 0.7);
        self.iris_size = self.iris_size.clamp(0.10, 0.22);
        self.pupil = self.pupil.clamp(0.2, 0.65);
        self.tilt = self.tilt.clamp(-0.2, 0.3);
        self.glow = self.glow.clamp(0., 1.);
        self.smoothing = self.smoothing.clamp(3., 30.);
        self.expression = self.expression.clamp(-1., 1.);
        for v in self.iris.iter_mut().chain(self.background.iter_mut()) {
            *v = v.clamp(0., 1.);
        }
    }
}

struct EyesApp {
    settings: Settings,
    receiver: tracking::Receiver,
    start: Instant,
    last: Instant,
    eyes: [[f32; 4]; 2],
    roll: f32,
    blinker: Blinker,
    springs: [Spring; 2],
    deformation: [[f32; 4]; 2],
    message: String,
    settings_path: PathBuf,
    screenshot: Option<PathBuf>,
    screenshot_requested: bool,
    exit_after: Option<f32>,
    control: Option<ControlHub>,
    external: [[f32; 4]; 2],
    external_roll: f32,
    revision: u64,
    rpc_status: String,
    asset_root: PathBuf,
    art: Art,
    gpu: Option<eframe::egui_wgpu::RenderState>,
    offscreen: Option<offscreen::Renderer>,
    output_mode: String,
    outputs: [String; 2],
    reload_requested: bool,
}
impl EyesApp {
    fn save_settings(&mut self) -> Result<(), String> {
        let contents = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
        std::fs::write(&self.settings_path, contents).map_err(|e| e.to_string())?;
        self.message = "Settings saved".into();
        Ok(())
    }

    fn reload_art(&mut self) -> Result<(), String> {
        let result = if let Some(gpu) = &self.gpu {
            render::install(gpu, &self.asset_root)
        } else if let Some(renderer) = &mut self.offscreen {
            renderer.reload(&self.asset_root)
        } else {
            assets::Pack::load(&self.asset_root).map(|pack| pack.art)
        };
        match result {
            Ok(art) => {
                self.settings.apply_art(&art);
                self.art = art;
                self.message = "Assets reloaded (including art defaults)".into();
                Ok(())
            }
            Err(e) => {
                self.message = format!("Reload failed; previous art retained: {e}");
                eprintln!("{}", self.message);
                Err(self.message.clone())
            }
        }
    }
    fn apply_config(&mut self, c: pb::Config, ctx: &egui::Context) -> Result<(), String> {
        if let Some(port) = c.tracking_port {
            self.receiver.try_rebind(port as u16)?;
        }
        if c.blink_interval.is_some() || c.blink_variation.is_some() {
            self.blinker.reschedule();
        }
        let s = &mut self.settings;
        if let Some(mode) = c.mode {
            s.mode = match pb::Mode::try_from(mode).unwrap() {
                pb::Mode::Idle => Mode::Idle,
                pb::Mode::Mouse => Mode::Mouse,
                pb::Mode::Ifacialmocap => Mode::Tracking,
                pb::Mode::External => Mode::External,
                pb::Mode::Simulator => Mode::Simulator,
            };
        }
        macro_rules! set {
            ($field:ident, $value:expr) => {
                if let Some(v) = $value {
                    s.$field = v;
                }
            };
        }
        set!(scale, c.scale);
        set!(separation, c.spacing);
        set!(iris_size, c.iris_size);
        set!(pupil, c.pupil);
        set!(tilt, c.tilt);
        set!(glow, c.glow);
        set!(expression, c.expression);
        set!(smoothing, c.response);
        set!(auto_blink, c.auto_blink);
        set!(mirror, c.mirror_tracking);
        set!(head_roll, c.follow_head_roll);
        set!(blink_interval, c.blink_interval);
        set!(blink_duration, c.blink_duration);
        set!(blink_variation, c.blink_variation);
        if let Some(port) = c.tracking_port {
            s.port = port as u16;
        }
        if let Some(ip) = c.phone_ip {
            s.phone = ip;
        }
        if let Some(j) = c.jelly {
            s.jelly.patch(j);
        }
        if let Some(l) = c.layers {
            s.layers.patch(l);
        }
        if let Some(sim) = c.simulator {
            s.simulator.patch(sim);
        }
        if let Some(art) = c.art {
            s.shape.patch(art);
        }
        if let Some(color) = c.iris {
            s.iris = [color.r, color.g, color.b];
        }
        if let Some(color) = c.background {
            s.background = [color.r, color.g, color.b];
        }
        if let Some(show) = c.show_controls {
            self.settings.show_controls = show;
        }
        if let Some(fullscreen) = c.fullscreen {
            self.settings.fullscreen = fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        }
        Ok(())
    }
    fn state(&self) -> pb::State {
        let s = &self.settings;
        let eye = |e: [f32; 4]| {
            Some(pb::Eye {
                gaze_x: e[0],
                gaze_y: e[1],
                openness: e[2],
                brow: e[3],
            })
        };
        pb::State {
            revision: self.revision,
            config: Some(s.rpc()),
            rendered_pose: Some(pb::Pose {
                left: eye(self.eyes[0]),
                right: eye(self.eyes[1]),
                roll: self.roll,
            }),
            tracking_live: self.receiver.live(),
            tracking_packets: self.receiver.packets,
            rejected_packets: self.receiver.rejected,
            jelly_left: self.deformation[0].to_vec(),
            jelly_right: self.deformation[1].to_vec(),
            tracking_status: self.receiver.status.clone(),
            output_mode: self.output_mode.clone(),
            left_output: self.outputs[0].clone(),
            right_output: self.outputs[1].clone(),
        }
    }
    fn new(
        cc: &eframe::CreationContext<'_>,
        settings_path: PathBuf,
        screenshot: Option<PathBuf>,
        exit_after: Option<f32>,
        socket: Option<PathBuf>,
        asset_root: PathBuf,
    ) -> assets::AssetResult<Self> {
        let gpu = cc
            .wgpu_render_state
            .as_ref()
            .ok_or("wgpu renderer required")?
            .clone();
        let art = render::install(&gpu, &asset_root)?;
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(17, 23, 35);
        style.visuals.selection.bg_fill = Color32::from_rgb(38, 88, 113);
        style.spacing.item_spacing = egui::vec2(10., 12.);
        cc.egui_ctx.set_style(style);
        let app = Self::without_window(
            settings_path,
            screenshot,
            exit_after,
            socket,
            asset_root,
            art,
            Some(gpu),
            None,
        )?;
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(app.settings.fullscreen));
        Ok(app)
    }
    #[allow(clippy::too_many_arguments)]
    fn without_window(
        settings_path: PathBuf,
        screenshot: Option<PathBuf>,
        exit_after: Option<f32>,
        socket: Option<PathBuf>,
        asset_root: PathBuf,
        art: Art,
        gpu: Option<eframe::egui_wgpu::RenderState>,
        offscreen: Option<offscreen::Renderer>,
    ) -> assets::AssetResult<Self> {
        let mut defaults = Settings::default();
        defaults.apply_art(&art);
        let (mut settings, mut message) = match std::fs::read_to_string(&settings_path) {
            Ok(s) => match serde_json::from_str::<Settings>(&s) {
                Ok(s) => (s, String::new()),
                Err(e) => (defaults.clone(), format!("Settings ignored: {e}")),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (defaults.clone(), String::new()),
            Err(e) => (defaults.clone(), format!("Cannot read settings: {e}")),
        };
        settings.sanitize();
        if let Err(e) = eyes::control::validate_config(&settings.rpc()) {
            settings = defaults;
            message = format!("Invalid saved settings; using defaults: {e}");
        }
        let receiver = tracking::Receiver::bind(settings.port);
        let (control, rpc_status) = match socket {
            Some(path) => match ControlHub::start(path.clone()) {
                Ok(hub) => (Some(hub), format!("gRPC / {}", path.display())),
                Err(e) => return Err(format!("gRPC 启动失败：{e}").into()),
            },
            None => (None, "gRPC disabled".into()),
        };
        eprintln!("{rpc_status}");
        Ok(Self {
            asset_root,
            art,
            gpu,
            offscreen,
            output_mode: "window".into(),
            outputs: [String::new(), String::new()],
            reload_requested: false,
            control,
            external: [[0., 0., 1., 0.]; 2],
            external_roll: 0.,
            revision: 0,
            rpc_status,
            settings,
            receiver,
            start: Instant::now(),
            last: Instant::now(),
            eyes: [[0., 0., 1., 0.]; 2],
            roll: 0.,
            blinker: Blinker::default(),
            springs: [Spring::default(), Spring::default()],
            deformation: [[0.; 4]; 2],
            message,
            settings_path,
            screenshot,
            screenshot_requested: false,
            exit_after,
        })
    }
    fn commands(
        &mut self,
        ctx: &egui::Context,
        t: f32,
    ) -> Vec<(tokio::sync::oneshot::Sender<Result<u64, String>>, u64)> {
        let mut replies = Vec::new();
        for _ in 0..64 {
            let pending = self
                .control
                .as_mut()
                .and_then(|hub| hub.commands.try_recv().ok());
            let Some(pending) = pending else { break };
            if pending.reply.is_closed() {
                continue;
            }
            match pending.command {
                Command::Pose(p) => {
                    for (dst, src) in self
                        .external
                        .iter_mut()
                        .zip([p.left.unwrap(), p.right.unwrap()])
                    {
                        *dst = [src.gaze_x, src.gaze_y, src.openness, src.brow];
                    }
                    self.external_roll = p.roll;
                    self.settings.mode = Mode::External;
                }
                Command::Configure(c) => {
                    if let Err(e) = self.apply_config(*c, ctx) {
                        let _ = pending.reply.send(Err(e));
                        continue;
                    }
                }
                Command::Blink => self
                    .blinker
                    .trigger(t, [true; 2], self.settings.blink_duration),
                Command::TriggerBlink(b) => self.blinker.trigger(
                    t,
                    [b.left.unwrap_or(true), b.right.unwrap_or(true)],
                    b.duration.unwrap_or(self.settings.blink_duration),
                ),
                Command::ConnectTracking => {
                    if let Err(e) = self.receiver.connect(&self.settings.phone) {
                        let _ = pending.reply.send(Err(e));
                        continue;
                    }
                }
                Command::SaveSettings => {
                    if let Err(e) = self.save_settings() {
                        let _ = pending.reply.send(Err(e));
                        continue;
                    }
                }
                Command::ReloadAssets => {
                    if let Err(e) = self.reload_art() {
                        let _ = pending.reply.send(Err(e));
                        continue;
                    }
                }
            }
            self.revision += 1;
            replies.push((pending.reply, self.revision));
        }
        replies
    }
    fn advance(&mut self, t: f32, dt: f32, mouse: [f32; 2]) {
        let live = self.settings.mode == Mode::Tracking && self.receiver.live();
        let mut target = [[0., 0., 1., 0.]; 2];
        let mut roll = 0.;
        if self.settings.mode == Mode::External {
            target = self.external;
            roll = self.external_roll;
        } else if self.settings.mode == Mode::Simulator {
            target = self.settings.simulator.pose(t);
            roll = self.settings.simulator.roll;
        } else if live {
            let f = self.receiver.face;
            for (i, eye) in target.iter_mut().enumerate() {
                let src = if self.settings.mirror { 1 - i } else { i };
                *eye = [
                    f.gaze[src][0] * if self.settings.mirror { -1. } else { 1. },
                    f.gaze[src][1],
                    ((1. - f.blink[src]) * (1. + 0.35 * f.wide[src] - 0.4 * f.squint[src]))
                        .clamp(0., 1.4),
                    f.brow[src],
                ];
            }
            if self.settings.head_roll {
                roll = f.roll * if self.settings.mirror { -0.4 } else { 0.4 };
            }
        } else {
            let gaze = if self.settings.mode == Mode::Mouse {
                mouse
            } else {
                [
                    (t * 0.57).sin() * 0.43 + (t * 1.31).sin() * 0.10,
                    (t * 0.41).sin() * 0.24,
                ]
            };
            for eye in &mut target {
                eye[0] = gaze[0];
                eye[1] = gaze[1];
            }
        }
        let automatic = self.settings.auto_blink && self.settings.mode != Mode::External && !live;
        self.blinker.apply(
            &mut target,
            t,
            automatic,
            self.settings.blink_interval,
            self.settings.blink_duration,
            self.settings.blink_variation,
        );
        let alpha = 1. - (-self.settings.smoothing * dt).exp();
        for (eye, dst) in self.eyes.iter_mut().zip(target) {
            for j in 0..4 {
                let a = if j == 2 {
                    1. - (-45. * dt).exp()
                } else {
                    alpha
                };
                eye[j] += (dst[j] - eye[j]) * a;
            }
        }
        self.roll += (roll - self.roll) * alpha;
        for i in 0..2 {
            self.deformation[i] = self.springs[i].step(self.eyes[i], &self.settings.jelly, dt);
        }
    }
    fn uniforms(&self, size: [f32; 2], t: f32) -> render::Uniforms {
        let s = &self.settings;
        let layer_flags = s.layers.uniforms();
        let art_shape = s.shape.uniforms();
        render::Uniforms {
            viewport: [size[0], size[1], t, s.scale],
            left: self.eyes[0],
            right: self.eyes[1],
            shape: [s.separation, s.iris_size, s.expression, self.roll],
            iris: [s.iris[0], s.iris[1], s.iris[2], 1.],
            background: [s.background[0], s.background[1], s.background[2], 1.],
            extra: [s.glow, s.pupil, s.tilt, 0.],
            layers_a: layer_flags[0],
            layers_b: layer_flags[1],
            jelly_left: self.deformation[0],
            jelly_right: self.deformation[1],
            contour: art_shape[0],
            detail: art_shape[1],
            placement: self.art.placement_uniforms(),
        }
    }
    fn controls(&mut self, ctx: &egui::Context, t: f32) {
        egui::SidePanel::left("controls").exact_width(276.).resizable(false).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(12.);
                ui.label(RichText::new("OCULUS / EYE STUDIO").size(19.).strong().color(Color32::from_rgb(151, 222, 244)));
                ui.label(RichText::new("Soft anime / Editable texture layers").small().weak());
                ui.separator();
                ui.label(RichText::new("01  MOTION SOURCE").small().strong());
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.settings.mode, Mode::Idle, "Idle");
                    ui.selectable_value(&mut self.settings.mode, Mode::Mouse, "Mouse");
                    ui.selectable_value(&mut self.settings.mode, Mode::Tracking, "Face");
                    ui.selectable_value(&mut self.settings.mode, Mode::External, "RPC");
                    ui.selectable_value(&mut self.settings.mode, Mode::Simulator, "Sim");
                });
                if self.settings.mode == Mode::Simulator {
                    ui.label("Look target  (-1..1, +X right / +Y down)");
                    let sim = &mut self.settings.simulator;
                    let (pad, response) = ui.allocate_exact_size(egui::vec2(240., 110.), egui::Sense::click_and_drag());
                    if (response.dragged() || response.clicked()) && let Some(pos) = response.interact_pointer_pos() {
                        sim.target_x = ((pos.x - pad.center().x) / (pad.width() * 0.5)).clamp(-1., 1.);
                        sim.target_y = ((pos.y - pad.center().y) / (pad.height() * 0.5)).clamp(-1., 1.);
                    }
                    ui.painter().rect_filled(pad, 4., Color32::from_rgb(28, 40, 55));
                    let stroke = egui::Stroke::new(1_f32, Color32::from_gray(80));
                    ui.painter().line_segment([pad.left_center(), pad.right_center()], stroke);
                    ui.painter().line_segment([pad.center_top(), pad.center_bottom()], stroke);
                    ui.painter().circle_filled(pad.center() + egui::vec2(sim.target_x * pad.width() * 0.5, sim.target_y * pad.height() * 0.5), 5., Color32::from_rgb(140, 220, 240));
                    ui.add(egui::Slider::new(&mut sim.target_x, -1.0..=1.0).text("Target X"));
                    ui.add(egui::Slider::new(&mut sim.target_y, -1.0..=1.0).text("Target Y"));
                    if ui.button("Look at center").clicked() { sim.target_x = 0.; sim.target_y = 0.; }
                    ui.checkbox(&mut sim.show_target, "Show target marker");
                    ui.checkbox(&mut sim.eye_motion, "Small eye movements / saccades");
                    ui.add(egui::Slider::new(&mut sim.motion_amplitude, 0.0..=0.3).text("Motion size"));
                    ui.add(egui::Slider::new(&mut sim.motion_speed, 0.1..=8.0).text("Motion Hz"));
                    ui.add(egui::Slider::new(&mut sim.convergence, 0.0..=0.3).text("Converge"));
                    ui.add(egui::Slider::new(&mut sim.left_openness, 0.0..=1.4).text("Left open"));
                    ui.add(egui::Slider::new(&mut sim.right_openness, 0.0..=1.4).text("Right open"));
                    ui.add(egui::Slider::new(&mut sim.left_brow, -1.0..=1.0).text("Left brow"));
                    ui.add(egui::Slider::new(&mut sim.right_brow, -1.0..=1.0).text("Right brow"));
                    ui.add(egui::Slider::new(&mut sim.roll, -0.8..=0.8).text("Roll (rad)"));
                }
                if self.settings.mode == Mode::Tracking {
                    ui.label("iPhone IP address"); ui.text_edit_singleline(&mut self.settings.phone);
                    ui.horizontal(|ui| {
                        ui.label("UDP"); ui.add(egui::DragValue::new(&mut self.settings.port).range(1024..=65535));
                        if ui.button("Listen").clicked() { self.receiver.rebind(self.settings.port); }
                        if ui.button("Connect").clicked() { let _ = self.receiver.connect(&self.settings.phone); }
                    });
                    ui.label(RichText::new(&self.receiver.status).small());
                    ui.checkbox(&mut self.settings.mirror, "Mirror tracking");
                    ui.checkbox(&mut self.settings.head_roll, "Follow head roll");
                }
                ui.checkbox(&mut self.settings.auto_blink, "Auto blink (idle / mouse / sim)");
                ui.collapsing("Blink timing", |ui| {
                    if ui.add(egui::Slider::new(&mut self.settings.blink_interval, 0.5..=20.0).text("Interval (s)")).changed() { self.blinker.reschedule(); }
                    ui.add(egui::Slider::new(&mut self.settings.blink_duration, 0.08..=1.0).text("Duration (s)"));
                    ui.add(egui::Slider::new(&mut self.settings.blink_variation, 0.0..=0.8).text("Variation"));
                    ui.horizontal(|ui| {
                        if ui.button("Wink left").clicked() { self.blinker.trigger(t, [true, false], self.settings.blink_duration); }
                        if ui.button("Wink right").clicked() { self.blinker.trigger(t, [false, true], self.settings.blink_duration); }
                    });
                });
                ui.label(RichText::new(&self.rpc_status).small().weak());
                ui.add(egui::Slider::new(&mut self.settings.smoothing, 3.0..=30.0).text("Response"));
                if ui.button("Blink  [Space]").clicked() { self.blinker.trigger(t, [true; 2], self.settings.blink_duration); }
                ui.separator();
                ui.label(RichText::new("02  EXPRESSION").small().strong());
                ui.checkbox(&mut self.settings.jelly.enabled, "Jelly eyes / spring deformation");
                ui.collapsing("Jelly tuning", |ui| {
                    ui.add(egui::Slider::new(&mut self.settings.jelly.strength, 0.0..=1.0).text("Strength"));
                    ui.add(egui::Slider::new(&mut self.settings.jelly.frequency, 1.0..=12.0).text("Spring Hz"));
                    ui.add(egui::Slider::new(&mut self.settings.jelly.damping, 0.15..=1.5).text("Damping"));
                });
                ui.horizontal(|ui| {
                    if ui.button("Calm").clicked() { self.settings.expression = 0.; }
                    if ui.button("Happy").clicked() { self.settings.expression = 1.; }
                    if ui.button("Sharp").clicked() { self.settings.expression = -1.; }
                });
                ui.add(egui::Slider::new(&mut self.settings.expression, -1.0..=1.0).text("Mood"));
                ui.separator(); ui.label(RichText::new("03  ART DIRECTION").small().strong());
                ui.horizontal(|ui| {
                    if ui.button("Azure").clicked() { self.settings.iris = [0.24, 0.65, 0.95]; }
                    if ui.button("Violet").clicked() { self.settings.iris = [0.66, 0.32, 0.94]; }
                    if ui.button("Original").clicked() { self.settings.iris = [1., 1., 1.]; }
                });
                ui.horizontal(|ui| { ui.color_edit_button_rgb(&mut self.settings.iris); ui.label("Iris"); ui.color_edit_button_rgb(&mut self.settings.background); ui.label("Backdrop"); });
                ui.add(egui::Slider::new(&mut self.settings.scale, 0.5..=1.5).text("Scale"));
                ui.add(egui::Slider::new(&mut self.settings.separation, 0.38..=0.7).text("Spacing"));
                ui.add(egui::Slider::new(&mut self.settings.iris_size, 0.10..=0.22).text("Iris size"));
                ui.add(egui::Slider::new(&mut self.settings.pupil, 0.2..=0.65).text("Pupil"));
                ui.add(egui::Slider::new(&mut self.settings.tilt, -0.2..=0.3).text("Eye tilt"));
                ui.add(egui::Slider::new(&mut self.settings.glow, 0.0..=1.0).text("Glow"));
                ui.collapsing("Eye shape (also controlled by RPC)", |ui| {
                    let a = &mut self.settings.shape;
                    ui.add(egui::Slider::new(&mut a.half_width, 0.2..=0.36).text("Half width"));
                    ui.add(egui::Slider::new(&mut a.upper_height, 0.08..=0.3).text("Upper lid"));
                    ui.add(egui::Slider::new(&mut a.lower_height, 0.06..=0.28).text("Lower lid"));
                    ui.add(egui::Slider::new(&mut a.roundness, 0.5..=1.5).text("Roundness"));
                    ui.add(egui::Slider::new(&mut a.iris_aspect, 0.6..=1.2).text("Iris aspect"));
                    ui.add(egui::Slider::new(&mut a.brow_height, 0.22..=0.5).text("Brow height"));
                    ui.add(egui::Slider::new(&mut a.lid_texture_height, 0.04..=0.25).text("Lid texture"));
                    ui.add(egui::Slider::new(&mut a.shadow_strength, 0.0..=0.5).text("Shadow"));
                });
                ui.separator();
                ui.label(RichText::new("04  VISIBLE LAYERS").small().strong());
                let layers = &mut self.settings.layers;
                ui.horizontal(|ui| { if ui.button("Show all").clicked() { *layers = Layers::default(); }
                    if ui.button("Hide all").clicked() { *layers = Layers { iris: false, pupil: false, highlights: false, sclera: false, upper_lid: false, lower_lid: false, brow: false }; } });
                egui::Grid::new("layers").num_columns(2).show(ui, |ui| {
                    for (n, (name, value)) in [("Iris", &mut layers.iris), ("Pupil", &mut layers.pupil), ("Highlights", &mut layers.highlights), ("Sclera", &mut layers.sclera), ("Upper lid / lashes", &mut layers.upper_lid), ("Lower lid", &mut layers.lower_lid), ("Brow", &mut layers.brow)].into_iter().enumerate() {
                        ui.checkbox(value, name); if n % 2 == 1 { ui.end_row(); }
                    }
                });
                ui.separator();
                ui.label(RichText::new("05  EXTERNAL ASSETS").small().strong());
                ui.label(RichText::new(self.asset_root.display().to_string()).small().weak());
                if ui.button("Reload textures + shape  [F5]").clicked() { self.reload_requested = true; }
                ui.horizontal(|ui| {
                    if ui.button("Save settings").clicked() && let Err(e) = self.save_settings() { self.message = format!("Save failed: {e}"); }
                    if ui.button("Reset").clicked() { self.settings = Settings::default(); self.settings.apply_art(&self.art); self.receiver.rebind(self.settings.port); self.blinker = Blinker::default(); ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false)); }
                });
                ui.label(RichText::new(&self.message).small());
                ui.label(RichText::new("Tab  hide controls\nF11  fullscreen    Esc  restore\nSpace  blink    1 / 2 / 3  expression").small().weak());
            });
        });
    }
}
impl eframe::App for EyesApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let t = self.start.elapsed().as_secs_f32();
        let dt = self.last.elapsed().as_secs_f32().min(0.1);
        self.last = Instant::now();
        self.receiver.poll();
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.reload_requested = true;
        }
        if self.reload_requested {
            self.reload_requested = false;
            let _ = self.reload_art();
        }
        let replies = self.commands(ctx, t);
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
                self.settings.show_controls = !self.settings.show_controls;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
                self.settings.fullscreen = !self.settings.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.settings.fullscreen));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.settings.fullscreen = false;
                self.settings.show_controls = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.blinker
                    .trigger(t, [true; 2], self.settings.blink_duration);
            }
            for (key, mood) in [
                (egui::Key::Num1, 0.),
                (egui::Key::Num2, 1.),
                (egui::Key::Num3, -1.),
            ] {
                if ctx.input(|i| i.key_pressed(key)) {
                    self.settings.expression = mood;
                }
            }
        }
        if self.settings.show_controls {
            self.controls(ctx, t);
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
            let live = self.settings.mode == Mode::Tracking && self.receiver.live();
            let mouse = ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p)).map(|p| [((p.x - rect.center().x) / (rect.width() * 0.4)).clamp(-1., 1.), ((p.y - rect.center().y) / (rect.height() * 0.4)).clamp(-1., 1.)]).unwrap_or([0.; 2]);
            self.advance(t, dt, mouse);
            let uniforms = self.uniforms([rect.width() * ctx.pixels_per_point(), rect.height() * ctx.pixels_per_point()], t);
            let s = &self.settings;
            ui.painter().add(eframe::egui_wgpu::Callback::new_paint_callback(rect, uniforms));
            if self.settings.show_controls && s.mode == Mode::Simulator && s.simulator.show_target {
                let pos = rect.center() + egui::vec2(s.simulator.target_x * rect.width() * 0.4, s.simulator.target_y * rect.height() * 0.4);
                let stroke = egui::Stroke::new(1_f32, Color32::from_rgb(86, 162, 184));
                ui.painter().circle_stroke(pos, 10., stroke);
                ui.painter().line_segment([pos - egui::vec2(16., 0.), pos + egui::vec2(16., 0.)], stroke);
                ui.painter().line_segment([pos - egui::vec2(0., 16.), pos + egui::vec2(0., 16.)], stroke);
            }
            if self.settings.show_controls {
                let badge = if live { format!("LIVE / UDP   |   {} frames", self.receiver.packets) } else if s.mode == Mode::External { format!("EXTERNAL / gRPC   |   revision {}", self.revision) } else if s.mode == Mode::Tracking { "WAITING FOR TRACKING / idle fallback".into() } else { "LIVE2D TEXTURES / LAYERED EYES".into() };
                ui.painter().text(rect.left_top() + egui::vec2(28., 28.), egui::Align2::LEFT_TOP, badge, egui::FontId::monospace(11.), Color32::from_rgb(118, 163, 184));
                ui.painter().text(rect.left_bottom() + egui::vec2(28., -24.), egui::Align2::LEFT_BOTTOM, "OCULUS     /     01                                      TAB  /  CLEAN OUTPUT", egui::FontId::monospace(10.), Color32::from_rgb(99, 120, 141));
            }
        });
        if let Some(hub) = &self.control {
            *hub.state.lock().unwrap() = self.state();
        }
        for (reply, revision) in replies {
            let _ = reply.send(Ok(revision));
        }
        // Reproducible GPU smoke capture without desktop automation.
        if self.screenshot.is_some() && t > 0.8 && !self.screenshot_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.screenshot_requested = true;
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(path) = self.screenshot.take()
            {
                let mut ppm =
                    format!("P6\n{} {}\n255\n", image.size[0], image.size[1]).into_bytes();
                for c in &image.pixels {
                    ppm.extend_from_slice(&[c.r(), c.g(), c.b()]);
                }
                match std::fs::write(path, ppm) {
                    Ok(()) => eprintln!("GPU screenshot saved"),
                    Err(e) => eprintln!("Screenshot failed: {e}"),
                }
            }
        }
        if self.exit_after.is_some_and(|limit| t > limit) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn main() -> eframe::Result {
    let options =
        service::Options::parse(std::env::args().skip(1)).map_err(eframe::Error::AppCreation)?;
    if options.help {
        service::help();
        return Ok(());
    }
    if options.list {
        #[cfg(target_os = "linux")]
        {
            drm_output::list(&options.device).map_err(eframe::Error::AppCreation)?;
            if let Some(path) = &options.right_device {
                drm_output::list(path).map_err(eframe::Error::AppCreation)?;
            }
        }
        #[cfg(not(target_os = "linux"))]
        return Err(eframe::Error::AppCreation(
            "DRM/KMS 设备查询需要 Linux".into(),
        ));
        #[cfg(target_os = "linux")]
        return Ok(());
    }
    if options.output != service::Output::Window {
        return service::run(options).map_err(eframe::Error::AppCreation);
    }
    eframe::run_native(
        "Oculus · Anime Eye Studio",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280., 760.])
                .with_min_inner_size([640., 400.]),
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(EyesApp::new(
                cc,
                options.settings,
                options.screenshot,
                options.exit_after,
                options.socket,
                options.assets,
            )?))
        }),
    )
}
