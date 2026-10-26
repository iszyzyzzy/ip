use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};
use eyes::assets::{Art, AssetResult, LAYERS, Pack, TEXTURE_SIZE};
use std::path::Path;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub viewport: [f32; 4], // width, height, time, scale
    pub left: [f32; 4],     // gaze x/y, openness, brow
    pub right: [f32; 4],
    pub shape: [f32; 4], // separation, iris size, expression, roll
    pub iris: [f32; 4],
    pub background: [f32; 4],
    pub extra: [f32; 4], // glow, pupil, tilt, reserved
    pub layers_a: [f32; 4],
    pub layers_b: [f32; 4],
    pub jelly_left: [f32; 4],
    pub jelly_right: [f32; 4],
    pub contour: [f32; 4],
    pub detail: [f32; 4],
    pub placement: [[f32; 4]; LAYERS + 2], // 各图层摆放 + 运动参数，来自 eye.json
}

pub struct Resources {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

pub fn install(state: &egui_wgpu::RenderState, root: &Path) -> AssetResult<Art> {
    let (art, resources) = create(&state.device, &state.queue, state.target_format, root)?;
    state.renderer.write().callback_resources.insert(resources);
    Ok(art)
}

pub fn create(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    root: &Path,
) -> AssetResult<(Art, Resources)> {
    let pack = Pack::load(root)?;
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Anime eyes WGSL"),
        source: wgpu::ShaderSource::Wgsl(pack.shader.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Eye parameters"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Procedural eye pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    });
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Eye uniforms"),
        contents: bytemuck::bytes_of(&Uniforms::zero()),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Editable eye layers"),
        size: wgpu::Extent3d {
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
            depth_or_array_layers: LAYERS as u32,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &pack.pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(TEXTURE_SIZE * 4),
            rows_per_image: Some(TEXTURE_SIZE),
        },
        texture.size(),
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    if let Some(error) = pollster::block_on(device.pop_error_scope()) {
        return Err(error.to_string().into());
    }
    Ok((
        pack.art,
        Resources {
            pipeline,
            uniform,
            bind,
        },
    ))
}

impl Resources {
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        parameters: &Uniforms,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(parameters));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl Uniforms {
    pub fn zero() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}
impl egui_wgpu::CallbackTrait for Uniforms {
    fn prepare(
        &self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &egui_wgpu::ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r = resources.get::<Resources>().unwrap();
        queue.write_buffer(&r.uniform, 0, bytemuck::bytes_of(self));
        Vec::new()
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let r = resources.get::<Resources>().unwrap();
        pass.set_pipeline(&r.pipeline);
        pass.set_bind_group(0, &r.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shader_is_valid() {
        let module =
            naga::front::wgsl::parse_str(include_str!("../assets/eyes.wgsl")).expect("WGSL parse");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("WGSL validation");
        assert_eq!(std::mem::size_of::<super::Uniforms>(), 352);
    }
}
