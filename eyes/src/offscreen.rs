use crate::render::{self, Resources, Uniforms};
use eyes::assets::{Art, AssetResult};
use std::path::Path;

struct Target {
    texture: wgpu::Texture,
    readback: wgpu::Buffer,
    stride: u32,
    size: (u32, u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "需要实际 GPU；显式运行检查表情渲染与左右镜像"]
    fn expressions_render_distinct_mirrored_shapes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let (renderer, art) = Renderer::new(&root, [(192, 192); 2]).unwrap();
        let mut u = Uniforms::zero();
        u.viewport[3] = 1.;
        u.background = [1., 0.945, 0.94, 1.];
        u.placement = art.placement_uniforms();
        let happy = renderer.render(1, u, 0).unwrap();
        u.background[3] = 2.;
        let excited = renderer.render(1, u, 0).unwrap();
        let other = renderer.render(0, u, 0).unwrap();
        assert!(happy.chunks_exact(4).filter(|p| p[0] < 128).count() > 100);
        assert!(excited.chunks_exact(4).filter(|p| p[0] < 128).count() > 100);
        let changed = happy
            .iter()
            .zip(&excited)
            .filter(|(a, b)| a.abs_diff(**b) > 30)
            .count();
        assert!(changed > 100);
        let to_image = |mut bytes: Vec<u8>| {
            for pixel in bytes.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            image::RgbaImage::from_raw(192, 192, bytes).unwrap()
        };
        let left = to_image(excited);
        let right = to_image(other);
        let mirrored = image::imageops::flip_horizontal(&left);
        let error: usize = mirrored
            .as_raw()
            .iter()
            .zip(right.as_raw())
            .map(|(a, b)| usize::from(a.abs_diff(*b)))
            .sum();
        assert!((error as f64 / right.as_raw().len() as f64) < 0.5);
        let mut preview = image::RgbaImage::new(384, 192);
        image::imageops::replace(&mut preview, &left, 0, 0);
        image::imageops::replace(&mut preview, &right, 192, 0);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("captures");
        std::fs::create_dir_all(&dir).unwrap();
        preview.save(dir.join("excited-preview.png")).unwrap();
    }

    #[test]
    #[ignore = "需要实际GPU；显式运行验证旋转方向及非正方形输出"]
    fn mounting_rotation_matches_rotated_render() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let (base, art) = Renderer::new(&root, [(192, 128); 2]).unwrap();
        let (turned, _) = Renderer::new(&root, [(128, 192); 2]).unwrap();
        let mut u = Uniforms::zero();
        u.viewport[3] = 1.;
        u.left = [0.2, -0.1, 0.65, 0.15];
        u.right = [-0.35, 0.15, 1., -0.1];
        u.shape[1] = 0.14;
        u.iris = [0.15, 0.7, 1., 0.];
        u.background = [0.01, 0.03, 0.06, 0.];
        u.extra[1] = 0.45;
        u.layers_a = [1.; 4];
        u.layers_b = [1.; 4];
        [u.contour, u.detail] = art.uniforms();
        u.placement = art.placement_uniforms();
        let captures = Path::new(env!("CARGO_MANIFEST_DIR")).join("captures/rotation-validation");
        std::fs::create_dir_all(&captures).unwrap();
        for eye in 0..2 {
            let pixels = base.render(eye, u, 0).unwrap();
            let original = image::RgbaImage::from_raw(192, 128, pixels).unwrap();
            assert!(original.pixels().any(|p| p[0].abs_diff(p[2]) > 30));
            for rotation in [90, 180, 270] {
                let expected = match rotation {
                    90 => image::imageops::rotate90(&original),
                    180 => image::imageops::rotate180(&original),
                    _ => image::imageops::rotate270(&original),
                };
                let renderer = if rotation == 180 { &base } else { &turned };
                let mut actual = renderer.render(eye, u, rotation).unwrap();
                let error: usize = actual
                    .iter()
                    .zip(expected.as_raw())
                    .map(|(a, b)| usize::from(a.abs_diff(*b)))
                    .sum();
                assert!(
                    (error as f64 / actual.len() as f64) < 0.1,
                    "eye={eye} rotation={rotation} error={error}"
                );
                if rotation == 90 {
                    for pixel in actual.as_chunks_mut::<4>().0 {
                        pixel.swap(0, 2);
                    }
                    image::save_buffer(
                        captures.join(format!("eye-{eye}-90.png")),
                        &actual,
                        128,
                        192,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    resources: [Resources; 2],
    targets: [Target; 2],
}

impl Renderer {
    pub fn new(root: &Path, sizes: [(u32, u32); 2]) -> AssetResult<(Self, Art)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))?;
        eprintln!("离屏渲染设备：{}", adapter.get_info().name);
        // 默认桌面限额可能超过 V3D 能力；渲染所需的绑定较少，纹理尺寸沿用实际适配器上限。
        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_limits: limits,
                ..Default::default()
            }))?;
        let (art, left) = render::create(&device, &queue, wgpu::TextureFormat::Bgra8Unorm, root)?;
        let (_, right) = render::create(&device, &queue, wgpu::TextureFormat::Bgra8Unorm, root)?;
        let targets = sizes.map(|size| {
            let stride = (size.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("单眼离屏纹理"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DRM 像素回读"),
                size: u64::from(stride) * u64::from(size.1),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            Target {
                texture,
                readback,
                stride,
                size,
            }
        });
        Ok((
            Self {
                device,
                queue,
                resources: [left, right],
                targets,
            },
            art,
        ))
    }

    pub fn reload(&mut self, root: &Path) -> AssetResult<Art> {
        // 两套资源都通过校验后再替换，避免热重载只更新了一只眼睛。
        let (art, left) = render::create(
            &self.device,
            &self.queue,
            wgpu::TextureFormat::Bgra8Unorm,
            root,
        )?;
        let (_, right) = render::create(
            &self.device,
            &self.queue,
            wgpu::TextureFormat::Bgra8Unorm,
            root,
        )?;
        self.resources = [left, right];
        Ok(art)
    }

    pub fn render(
        &self,
        eye: usize,
        mut uniforms: Uniforms,
        rotation: u16,
    ) -> AssetResult<Vec<u8>> {
        let target = &self.targets[eye];
        uniforms.viewport[0] = target.size.0 as f32;
        uniforms.viewport[1] = target.size.1 as f32;
        uniforms.shape[0] = 0.;
        uniforms.extra[3] = if eye == 0 { 1. } else { -1. };
        // 安装方向在最终采样坐标补偿，保留眼动和头部滚转自己的坐标系。
        uniforms.iris[3] = f32::from(rotation / 90);
        let view = target.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("独立眼睛渲染"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.resources[eye].draw(&self.queue, &uniforms, &mut pass);
        }
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &target.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(target.stride),
                    rows_per_image: Some(target.size.1),
                },
            },
            target.texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        target
            .readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let mapped = target.readback.slice(..).get_mapped_range();
        let mut pixels = Vec::with_capacity((target.size.0 * target.size.1 * 4) as usize);
        for row in mapped.chunks_exact(target.stride as usize) {
            pixels.extend_from_slice(&row[..target.size.0 as usize * 4]);
        }
        drop(mapped);
        target.readback.unmap();
        Ok(pixels)
    }
}
