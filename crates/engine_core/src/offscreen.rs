use anyhow::Result;
use glam::{Mat4, Vec2};

use crate::renderer::SpriteVertex;
use crate::state::EngineState;

pub struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    render_pipeline: wgpu::RenderPipeline,
    // kept to maintain bind group backing; not otherwise read
    _uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    white_texture_view: wgpu::TextureView,
    white_sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl OffscreenRenderer {
    pub async fn new(width: u32, height: u32) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No adapter"))?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("offscreen_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        // White 1x1 texture
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("white"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let white_texture_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::ImageCopyTexture { aspect: wgpu::TextureAspect::All, texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO },
            &[255, 255, 255, 255],
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let white_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // Uniforms and pipeline
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("uniform"), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }],
            label: Some("ubl")
        });
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
            label: Some("tbl"),
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor { layout: &uniform_bind_group_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }], label: Some("ubg") });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("sprite"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sprite.wgsl").into()) });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("pl"), bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout], push_constant_ranges: &[] });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[SpriteVertex::desc()], compilation_options: wgpu::PipelineCompilationOptions::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: wgpu::PipelineCompilationOptions::default() }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("vb"), size: (std::mem::size_of::<SpriteVertex>() * 4 * 1024) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ib"), size: (std::mem::size_of::<u16>() * 6 * 1024) as u64, usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        // Write projection
        let proj = Mat4::orthographic_lh(0.0, width as f32, 0.0, height as f32, -1000.0, 1000.0);
        queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&proj.to_cols_array()));

        Ok(Self { device, queue, format, width, height, render_pipeline, _uniform_buffer: uniform_buffer, uniform_bind_group, texture_bind_group_layout, white_texture_view, white_sampler, vertex_buffer, index_buffer })
    }

    pub fn render_state_to_rgba(&self, state: &EngineState) -> Result<Vec<u8>> {
        // Build geometry from state (white texture + tint from sprite color)
        let mut vertices: Vec<SpriteVertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let mut count = 0u16;
        let sprites = state.get_sprites();
        let trans = state.get_transforms();
        // Build a map from entity_id to transform
        let mut tmap = std::collections::HashMap::new();
        for chunk in trans.chunks_exact(6) { tmap.insert(chunk[0] as u32, (chunk[1], chunk[2], chunk[3], chunk[4], chunk[5])); }
        for sd in sprites.iter() {
            if let Some(&(x,y,rot,sx,sy)) = tmap.get(&sd.entity_id) {
                let pos = Vec2::new(x, y);
                let scale = Vec2::new(sx, sy); // Use direct virtual canvas coordinates
                let half = scale * 0.5;
                let cosr = (rot as f32).cos();
                let sinr = (rot as f32).sin();
                let corners = [Vec2::new(-half.x, -half.y), Vec2::new(half.x, -half.y), Vec2::new(half.x, half.y), Vec2::new(-half.x, half.y)];
                let uvs = [Vec2::new(sd.uv[0], sd.uv[1]), Vec2::new(sd.uv[2], sd.uv[1]), Vec2::new(sd.uv[2], sd.uv[3]), Vec2::new(sd.uv[0], sd.uv[3])];
                for i in 0..4 {
                    let c = corners[i];
                    let rotp = Vec2::new(c.x * cosr - c.y * sinr, c.x * sinr + c.y * cosr);
                    let wp = pos + rotp;
                    vertices.push(SpriteVertex { position: [wp.x, wp.y, 0.0], tex_coords: [uvs[i].x, uvs[i].y], color: sd.color });
                }
                let base = count;
                indices.extend_from_slice(&[base, base+1, base+2, base+2, base+3, base]);
                count += 4;
            }
        }

        // Upload
        if !vertices.is_empty() {
            self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            self.queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));
        }

        // Create offscreen target
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_target"),
            size: wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("offscreen_encoder") });
        let cc = state.get_clear_color();
        let clear = wgpu::Color { r: cc[0] as f64, g: cc[1] as f64, b: cc[2] as f64, a: cc[3] as f64 };
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("offscreen_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            if !vertices.is_empty() {
                rpass.set_pipeline(&self.render_pipeline);
                rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                let white_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.white_texture_view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.white_sampler) },
                    ],
                    label: Some("white_bg"),
                });
                rpass.set_bind_group(1, &white_bg, &[]);
                rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }
        }

        // Readback
        let bytes_per_pixel = 4u32;
        let output_size = (self.width * self.height * bytes_per_pixel) as u64;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyBuffer { buffer: &staging, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(self.width * bytes_per_pixel), rows_per_image: Some(self.height) } },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = staging.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, move |_| {});
        self.device.poll(wgpu::Maintain::Wait);
        let data = buffer_slice.get_mapped_range().to_vec();
        let _ = buffer_slice;
        staging.unmap();
        Ok(data)
    }
}
