use serde::Deserialize;
use std::{
    error::Error,
    path::{Path, PathBuf},
};
pub type AssetResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
pub const TEXTURE_SIZE: u32 = 512;
pub const LAYERS: usize = 7;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub iris_size: f32,
    pub pupil: f32,
    pub tilt: f32,
    pub glow: f32,
    pub iris: [f32; 3],
    pub background: [f32; 3],
}
/// 单个图层在眼睛画布上的摆放。offset 以贴图宽度为单位（0.1 = 512 图上的 51 像素），
/// x 向外眼角、y 向下为正；scale 以图层中心缩放；
/// gaze 为跟随视线的比例：虹膜、瞳孔、高光通常为 1，眼白和眼线为 0。
#[derive(Clone, Copy, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Placement {
    pub offset: [f32; 2],
    pub scale: f32,
    pub gaze: f32,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            offset: [0., 0.],
            scale: 1.,
            gaze: 0.,
        }
    }
}

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Placements {
    pub iris: Placement,
    pub pupil: Placement,
    pub highlights: Placement,
    pub sclera: Placement,
    pub upper_lid: Placement,
    pub lower_lid: Placement,
    pub brow: Placement,
}

/// 纯贴图模式下的运动参数：眨眼时内层以 blink_pivot 为轴纵向压扁，
/// 上下眼线从各自所在高度平移到 blink_pivot，合成闭眼线。
#[derive(Clone, Copy, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Motion {
    /// 一张贴图在屏幕上的半边长；屏幕高度为 1。所有图层共用同一画布。
    pub canvas: f32,
    pub gaze_range: [f32; 2],
    pub blink_pivot: f32,
    pub upper_lid_y: f32,
    pub lower_lid_y: f32,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            canvas: 0.4,
            gaze_range: [0.075, 0.055],
            blink_pivot: 0.05,
            upper_lid_y: -0.14,
            lower_lid_y: 0.13,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Art {
    pub half_width: f32,
    pub upper_height: f32,
    pub lower_height: f32,
    pub roundness: f32,
    pub iris_aspect: f32,
    pub brow_height: f32,
    pub lid_texture_height: f32,
    pub shadow_strength: f32,
    pub defaults: Defaults,
    pub textures: [String; LAYERS],
    #[serde(default)]
    pub placement: Placements,
    #[serde(default)]
    pub motion: Motion,
}
impl Art {
    pub fn validate(&self) -> AssetResult<()> {
        let d = &self.defaults;
        for (name, v, lo, hi) in [
            ("half_width", self.half_width, 0.2, 0.36),
            ("upper_height", self.upper_height, 0.08, 0.3),
            ("lower_height", self.lower_height, 0.06, 0.28),
            ("roundness", self.roundness, 0.5, 1.5),
            ("iris_aspect", self.iris_aspect, 0.6, 1.2),
            ("brow_height", self.brow_height, 0.22, 0.5),
            ("lid_texture_height", self.lid_texture_height, 0.04, 0.25),
            ("shadow_strength", self.shadow_strength, 0., 0.5),
            ("iris_size", d.iris_size, 0.1, 0.22),
            ("pupil", d.pupil, 0.2, 0.65),
            ("tilt", d.tilt, -0.2, 0.3),
            ("glow", d.glow, 0., 1.),
        ] {
            if !v.is_finite() || !(lo..=hi).contains(&v) {
                return Err(format!("eye.json: {name} must be in [{lo}, {hi}]").into());
            }
        }
        for (name, l) in self.layer_placements() {
            let ok = l.offset.iter().all(|v| v.is_finite() && v.abs() <= 1.)
                && l.scale.is_finite()
                && (0.1..=4.).contains(&l.scale)
                && l.gaze.is_finite()
                && l.gaze.abs() <= 2.;
            if !ok {
                return Err(format!(
                    "eye.json: placement.{name} needs offset in [-1, 1], scale in [0.1, 4], gaze in [-2, 2]"
                )
                .into());
            }
        }
        let m = &self.motion;
        let motion_ok = m.canvas.is_finite()
            && (0.1..=1.).contains(&m.canvas)
            && m.gaze_range
                .iter()
                .all(|v| v.is_finite() && (0.0..=0.3).contains(v))
            && [m.blink_pivot, m.upper_lid_y, m.lower_lid_y]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 0.5);
        if !motion_ok {
            return Err("eye.json: motion.canvas must be in [0.1, 1], gaze_range in [0, 0.3], other motion values in [-0.5, 0.5]".into());
        }
        for v in d.iris.iter().chain(&d.background) {
            if !v.is_finite() || !(0.0..=1.0).contains(v) {
                return Err("eye.json: colors must be in [0, 1]".into());
            }
        }
        Ok(())
    }
    pub fn uniforms(&self) -> [[f32; 4]; 2] {
        [
            [
                self.half_width,
                self.upper_height,
                self.lower_height,
                self.roundness,
            ],
            [
                self.iris_aspect,
                self.brow_height,
                self.lid_texture_height,
                self.shadow_strength,
            ],
        ]
    }
}

impl Art {
    fn layer_placements(&self) -> [(&'static str, Placement); LAYERS] {
        let p = &self.placement;
        // 顺序与 textures 数组和着色器中的图层索引一致。
        [
            ("iris", p.iris),
            ("pupil", p.pupil),
            ("highlights", p.highlights),
            ("sclera", p.sclera),
            ("upper_lid", p.upper_lid),
            ("lower_lid", p.lower_lid),
            ("brow", p.brow),
        ]
    }
    /// 前 7 项为各图层 offset.xy、scale、gaze；后两项为运动参数。
    pub fn placement_uniforms(&self) -> [[f32; 4]; LAYERS + 2] {
        let mut out = [[0.; 4]; LAYERS + 2];
        for (slot, (_, l)) in out.iter_mut().zip(self.layer_placements()) {
            *slot = [l.offset[0], l.offset[1], l.scale, l.gaze];
        }
        let m = &self.motion;
        out[LAYERS] = [m.gaze_range[0], m.gaze_range[1], m.blink_pivot, m.canvas];
        out[LAYERS + 1] = [m.upper_lid_y, m.lower_lid_y, 0., 0.];
        out
    }
}

pub struct Pack {
    pub art: Art,
    pub shader: String,
    pub pixels: Vec<u8>,
}
impl Pack {
    pub fn load(root: &Path) -> AssetResult<Self> {
        let path = root.join("eye.json");
        let art: Art = serde_json::from_slice(
            &std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        )?;
        art.validate()?;
        let shader = std::fs::read_to_string(root.join("eyes.wgsl"))?;
        let module =
            naga::front::wgsl::parse_str(&shader).map_err(|e| e.emit_to_string(&shader))?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)?;
        let mut pixels = Vec::with_capacity((TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize * LAYERS);
        for name in &art.textures {
            let path = root.join(name);
            pixels.extend(load_texture(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        Ok(Self {
            art,
            shader,
            pixels,
        })
    }
}

/// Convert editable SVG or RGBA PNG into straight-alpha RGBA8, one atlas layer.
pub fn load_texture(path: &Path) -> AssetResult<Vec<u8>> {
    if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("svg"))
    {
        let tree =
            resvg::usvg::Tree::from_data(&std::fs::read(path)?, &resvg::usvg::Options::default())?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(TEXTURE_SIZE, TEXTURE_SIZE)
            .ok_or("texture allocation failed")?;
        let transform = resvg::tiny_skia::Transform::from_scale(
            TEXTURE_SIZE as f32 / tree.size().width(),
            TEXTURE_SIZE as f32 / tree.size().height(),
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let mut data = pixmap.data().to_vec();
        for px in data.as_chunks_mut::<4>().0 {
            let alpha = px[3] as u32;
            for c in &mut px[..3] {
                *c = (*c as u32 * 255 + alpha / 2)
                    .checked_div(alpha)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
        Ok(data)
    } else if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        reader.limits(limits);
        let img = reader.decode()?.to_rgba8();
        Ok(image::imageops::resize(
            &img,
            TEXTURE_SIZE,
            TEXTURE_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
        .into_raw())
    } else {
        Err("expected SVG or PNG".into())
    }
}

pub fn find_root() -> PathBuf {
    // Development uses the project pack; distributed app finds its sibling pack.
    let cwd = PathBuf::from("assets");
    if cwd.join("eye.json").is_file() {
        return cwd;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("assets")))
        .unwrap_or(cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_pack_is_complete_and_transparent() {
        let pack = Pack::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap();
        assert_eq!(
            pack.pixels.len(),
            TEXTURE_SIZE as usize * TEXTURE_SIZE as usize * 4 * LAYERS
        );
        assert_eq!(pack.pixels[3], 0); // iris corner transparent
        assert!(pack.pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
        let mut art = pack.art;
        art.roundness = 0.;
        assert!(art.validate().is_err());
    }
}
