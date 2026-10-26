//! cargo run --example export_textures -- assets output/textures
//! Exports the source SVG pack into independently editable RGBA PNG layers.
use eyes::assets::{TEXTURE_SIZE, load_texture};
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| "assets".into());
    let out = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| "output/textures".into());
    std::fs::create_dir_all(&out)?;
    for name in [
        "iris",
        "pupil",
        "highlights",
        "sclera",
        "upper_lid",
        "lower_lid",
        "brow",
    ] {
        let pixels = load_texture(&root.join(format!("{name}.svg")))?;
        let path = out.join(format!("{name}.png"));
        image::save_buffer(
            &path,
            &pixels,
            TEXTURE_SIZE,
            TEXTURE_SIZE,
            image::ColorType::Rgba8,
        )?;
        println!("{}", path.display());
    }
    Ok(())
}
