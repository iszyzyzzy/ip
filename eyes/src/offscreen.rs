use crate::render::{self, Resources, Uniforms};
use eyes::assets::{Art, AssetResult};
use std::path::Path;

struct Target {
    texture: wgpu::Texture,
    readback: wgpu::Buffer,
    stride: u32,
    size: (u32, u32),
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

    pub fn render(&self, eye: usize, mut uniforms: Uniforms) -> AssetResult<Vec<u8>> {
        let target = &self.targets[eye];
        uniforms.viewport[0] = target.size.0 as f32;
        uniforms.viewport[1] = target.size.1 as f32;
        uniforms.shape[0] = 0.;
        uniforms.extra[3] = if eye == 0 { 1. } else { -1. };
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
