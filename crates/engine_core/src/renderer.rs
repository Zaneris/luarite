use anyhow::Result;
use glam::{Mat4, Vec2, Vec4};
use image::GenericImageView;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

/// 2D sprite vertex for batched rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
}

impl SpriteVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Sprite instance data for v2 flat array format
#[derive(Debug, Clone)]
pub struct SpriteInstance {
    pub entity_id: u32,
    pub texture_id: u32,
    pub position: Vec2,
    pub rotation: f32,
    pub size: Vec2,
    pub uv_rect: Vec4, // (u0, v0, u1, v1)
    pub color: Vec4,   // (r, g, b, a)
}

/// Transform data for v2 flat array format
#[derive(Debug, Clone)]
pub struct Transform {
    pub entity_id: u32,
    pub position: Vec2,
    pub rotation: f32,
    pub size: Vec2,
}

/// Texture handle for the renderer
#[derive(Debug)]
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }
}

/// 2D Sprite Renderer optimized for batching
pub struct SpriteRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    surface: Option<wgpu::Surface<'static>>,
    render_pipeline: wgpu::RenderPipeline,

    // Sprite batching resources
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    // Separate uniform buffer for presentation pass to avoid conflicts
    present_uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    present_uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,

    // Sprite batch data
    #[allow(dead_code)]
    max_sprites: u32,
    sprite_vertices: Vec<SpriteVertex>,
    sprite_indices: Vec<u16>,

    // Textures
    textures: Vec<Option<Texture>>,
    texture_bind_groups: Vec<Option<wgpu::BindGroup>>,
    white_texture: Texture,

    // Batches for per-texture draws
    batches: Vec<DrawBatch>,
    last_draw_calls: u32,

    // Transforms for entity management
    transforms: std::collections::HashMap<u32, Transform>,

    // HUD overlay
    hud_texture: Option<Texture>,
    hud_bind_group: Option<wgpu::BindGroup>,
    hud_size: (u32, u32),
    hud_scale: f32,

    // Clear/background color
    clear_color: wgpu::Color,

    // Virtual scene render target and samplers
    virtual_mode: crate::state::VirtualResolution,
    scene_texture: Option<Texture>,
    scene_bind_group: Option<wgpu::BindGroup>,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
    virtual_size: (u32, u32),
}

impl SpriteRenderer {
    // Headless constructor for testing - no window/surface required
    pub async fn new_headless(width: u32, height: u32) -> Result<Self> {
        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("headless_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        // Use a standard format for headless rendering
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        // Create a dummy config for headless mode
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Self::new_common(device, queue, config, None).await
    }

    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Self::new_common(device, queue, config, Some(surface)).await
    }

    async fn new_common(
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        surface: Option<wgpu::Surface<'static>>,
    ) -> Result<Self> {
        // Create white texture for solid color sprites
        let white_texture = Self::create_white_texture(&device, &queue)?;

        let max_sprites = 10000; // Match capability from API

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sprite.wgsl").into()),
        });

        // Create uniform buffer for projection matrix
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform_buffer"),
            size: 64, // 4x4 f32 matrix
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        // Create separate uniform buffer for presentation pass
        let present_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("present_uniform_buffer"),
            size: std::mem::size_of::<[[f32; 4]; 4]>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let present_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: present_uniform_buffer.as_entire_binding(),
            }],
            label: Some("present_uniform_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render_pipeline_layout"),
                bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SpriteVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Create vertex and index buffers
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_vertex_buffer"),
            size: (max_sprites * 4 * std::mem::size_of::<SpriteVertex>() as u32) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_index_buffer"),
            size: (max_sprites * 6 * std::mem::size_of::<u16>() as u32) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut textures = Vec::with_capacity(1000); // Match max_textures capability
        textures.resize_with(1000, || None);
        let mut texture_bind_groups = Vec::with_capacity(1000);
        texture_bind_groups.resize_with(1000, || None);

        // Common samplers used for presenting the virtual scene
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Ok(Self {
            device,
            queue,
            config,
            surface,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            present_uniform_buffer,
            present_uniform_bind_group,
            texture_bind_group_layout,
            max_sprites,
            sprite_vertices: Vec::with_capacity(max_sprites as usize * 4),
            sprite_indices: Vec::with_capacity(max_sprites as usize * 6),
            textures,
            texture_bind_groups,
            white_texture,
            batches: Vec::with_capacity(64),
            last_draw_calls: 0,
            transforms: std::collections::HashMap::new(),
            hud_texture: None,
            hud_bind_group: None,
            hud_size: (0, 0),
            hud_scale: 2.0,
            clear_color: wgpu::Color::BLACK,
            virtual_mode: crate::state::VirtualResolution::Hd1920x1080,
            scene_texture: None,
            scene_bind_group: None,
            nearest_sampler,
            linear_sampler,
            virtual_size: (1920, 1080),
        })
    }

    fn create_white_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Texture> {
        let white_pixels = [255u8; 4]; // RGBA white pixel (so sprites show as their tint colors)
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba(white_pixels));
        let dynamic_img = image::DynamicImage::ImageRgba8(img);
        Texture::from_image(device, queue, &dynamic_img, Some("white_texture"))
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, &self.config);
            }
            self.update_projection_matrix();
        }
    }

    fn update_projection_matrix(&self) {
        self.update_projection_matrix_for(self.config.width, self.config.height);
    }

    fn update_projection_matrix_for(&self, w: u32, h: u32) {
        let projection = Mat4::orthographic_lh(0.0, w as f32, 0.0, h as f32, -1000.0, 1000.0);
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&projection.to_cols_array()),
        );
    }

    pub fn load_texture(&mut self, texture_id: u32, bytes: &[u8], label: &str) -> Result<()> {
        if texture_id as usize >= self.textures.len() {
            return Err(anyhow::anyhow!(
                "Texture ID {} exceeds maximum texture count",
                texture_id
            ));
        }

        let texture = Texture::from_bytes(&self.device, &self.queue, bytes, label)?;
        self.textures[texture_id as usize] = Some(texture);
        tracing::debug!("Loaded texture {} with ID {}", label, texture_id);
        Ok(())
    }

    pub fn set_transforms_v2(&mut self, transforms: &[f32]) -> Result<()> {
        if transforms.len() % 6 != 0 {
            return Err(anyhow::anyhow!(
                "Transform array must have stride of 6 (id, x, y, rot, w, h)"
            ));
        }

        for chunk in transforms.chunks_exact(6) {
            let entity_id = chunk[0] as u32;
            let transform = Transform {
                entity_id,
                position: Vec2::new(chunk[1], chunk[2]),
                rotation: chunk[3],
                size: Vec2::new(chunk[4], chunk[5]),
            };
            self.transforms.insert(entity_id, transform);
        }

        Ok(())
    }

    pub fn submit_sprites_v2(&mut self, _sprite_count: u32) -> Result<()> {
        self.sprite_vertices.clear();
        self.sprite_indices.clear();

        Ok(())
    }

    fn add_sprite_to_batch(&mut self, sprite: SpriteInstance, sprite_idx: usize) -> Result<()> {
        let half_size = sprite.size * 0.5;

        // Calculate sprite corners with rotation
        let cos_rot = sprite.rotation.cos();
        let sin_rot = sprite.rotation.sin();

        let corners = [
            Vec2::new(-half_size.x, -half_size.y), // Top-left
            Vec2::new(half_size.x, -half_size.y),  // Top-right
            Vec2::new(half_size.x, half_size.y),   // Bottom-right
            Vec2::new(-half_size.x, half_size.y),  // Bottom-left
        ];

        let uvs = [
            Vec2::new(sprite.uv_rect.x, sprite.uv_rect.y), // Top-left
            Vec2::new(sprite.uv_rect.z, sprite.uv_rect.y), // Top-right
            Vec2::new(sprite.uv_rect.z, sprite.uv_rect.w), // Bottom-right
            Vec2::new(sprite.uv_rect.x, sprite.uv_rect.w), // Bottom-left
        ];

        // Add vertices
        for i in 0..4 {
            let corner = corners[i];
            let rotated = Vec2::new(
                corner.x * cos_rot - corner.y * sin_rot,
                corner.x * sin_rot + corner.y * cos_rot,
            );
            let world_pos = sprite.position + rotated;

            self.sprite_vertices.push(SpriteVertex {
                position: [world_pos.x, world_pos.y, 0.0],
                tex_coords: [uvs[i].x, uvs[i].y],
                color: sprite.color.to_array(),
            });
        }

        // Add indices (two triangles per quad)
        let base_idx = (sprite_idx * 4) as u16;
        self.sprite_indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx + 2,
            base_idx + 3,
            base_idx,
        ]);

        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        let surface = self.surface.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot render to screen without a surface (use new() instead of new_headless())"
            )
        })?;
        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Ensure scene texture exists before rendering
        if self.scene_texture.is_none() {
            let dummy_state = crate::state::EngineState::new();
            self.ensure_scene_texture(&dummy_state)?;
        }

        // Upload sprite vertex/index data if it exists
        if !self.sprite_vertices.is_empty() {
            self.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&self.sprite_vertices),
            );
            self.queue.write_buffer(
                &self.index_buffer,
                0,
                bytemuck::cast_slice(&self.sprite_indices),
            );
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        // Step 1: Render sprites to the virtual canvas (offscreen texture)
        if let Some(scene) = &self.scene_texture {
            let (vw, vh) = self.virtual_size;
            let projection_virtual =
                Mat4::orthographic_lh(0.0, vw as f32, 0.0, vh as f32, -1000.0, 1000.0);
            self.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&projection_virtual.to_cols_array()),
            );

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("virtual_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &scene.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if !self.sprite_vertices.is_empty() {
                pass.set_pipeline(&self.render_pipeline);
                pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

                // This bind group is only needed if there are sprites with no texture
                let white_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.white_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.white_texture.sampler),
                        },
                    ],
                    label: Some("white_bg"),
                });

                for batch in &self.batches {
                    if let Some(bg) = self.get_bind_group(batch.texture_id) {
                        pass.set_bind_group(1, bg, &[]);
                    } else {
                        pass.set_bind_group(1, &white_bind_group, &[]);
                    }
                    pass.draw_indexed(
                        batch.start_index..(batch.start_index + batch.index_count),
                        0,
                        0..1,
                    );
                }
            }
            drop(pass);
        }

        // Step 2: Present the virtual canvas and HUD to the screen in a single pass
        let projection_identity = Mat4::IDENTITY;
        self.queue.write_buffer(
            &self.present_uniform_buffer,
            0,
            bytemuck::cast_slice(&projection_identity.to_cols_array()),
        );

        let mut present_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), // Letterbox color
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        present_pass.set_pipeline(&self.render_pipeline);
        present_pass.set_bind_group(0, &self.present_uniform_bind_group, &[]);

        // Draw the virtual canvas (letterboxed)
        if let Some(scene) = &self.scene_texture {
            let (window_w, window_h) = (self.config.width as f32, self.config.height as f32);
            let (virtual_w, virtual_h) = (self.virtual_size.0 as f32, self.virtual_size.1 as f32);

            let scale = (window_w / virtual_w).min(window_h / virtual_h);

            let (final_scale, use_linear_scaling) = match self.virtual_mode {
                crate::state::VirtualResolution::Retro320x180 => (scale.floor().max(1.0), false),
                crate::state::VirtualResolution::Hd1920x1080 => {
                    let ideal_scale = (scale * 100.0).round() / 100.0;
                    let nearest_int_scale = scale.round();
                    if (ideal_scale - nearest_int_scale).abs() < 0.05 {
                        (nearest_int_scale, false) // Use integer scaling
                    } else {
                        (scale, true) // Use linear scaling
                    }
                }
            };

            let scaled_w = virtual_w * final_scale;
            let scaled_h = virtual_h * final_scale;
            let offset_x = (window_w - scaled_w) / 2.0;
            let offset_y = (window_h - scaled_h) / 2.0;

            let ndc_left = (offset_x / window_w) * 2.0 - 1.0;
            let ndc_right = ((offset_x + scaled_w) / window_w) * 2.0 - 1.0;
            let ndc_top = 1.0 - (offset_y / window_h) * 2.0;
            let ndc_bottom = 1.0 - ((offset_y + scaled_h) / window_h) * 2.0;

            let verts = [
                SpriteVertex {
                    position: [ndc_left, ndc_bottom, 0.0],
                    tex_coords: [0.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                SpriteVertex {
                    position: [ndc_right, ndc_bottom, 0.0],
                    tex_coords: [1.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                SpriteVertex {
                    position: [ndc_right, ndc_top, 0.0],
                    tex_coords: [1.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                SpriteVertex {
                    position: [ndc_left, ndc_top, 0.0],
                    tex_coords: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
            ];
            let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

            let sampler = if use_linear_scaling {
                &self.linear_sampler
            } else {
                &self.nearest_sampler
            };
            let scene_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&scene.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
                label: Some("scene_bg_with_sampler"),
            });

            present_pass.set_bind_group(1, &scene_bg, &[]);

            let vbuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("present_vbuf"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ibuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("present_ibuf"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            present_pass.set_vertex_buffer(0, vbuf.slice(..));
            present_pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint16);
            present_pass.draw_indexed(0..6, 0, 0..1);
        }

        // Draw the HUD on top
        if let Some(hud_bg) = self.hud_bind_group.as_ref() {
            let (hud_w, hud_h) = (self.hud_size.0 as f32, self.hud_size.1 as f32);
            let (win_w, win_h) = (self.config.width as f32, self.config.height as f32);

            // Position HUD at top-left. Convert pixel coordinates to NDC.
            let x = 10.0; // 10px from left
            let y = 10.0; // 10px from top
            let w = hud_w * self.hud_scale;
            let h = hud_h * self.hud_scale;

            let ndc_left = (x / win_w) * 2.0 - 1.0;
            let ndc_right = ((x + w) / win_w) * 2.0 - 1.0;
            let ndc_top = 1.0 - (y / win_h) * 2.0;
            let ndc_bottom = 1.0 - ((y + h) / win_h) * 2.0;

            let verts = [
                SpriteVertex {
                    position: [ndc_left, ndc_bottom, 0.0],
                    tex_coords: [0.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                SpriteVertex {
                    position: [ndc_right, ndc_bottom, 0.0],
                    tex_coords: [1.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                SpriteVertex {
                    position: [ndc_right, ndc_top, 0.0],
                    tex_coords: [1.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                SpriteVertex {
                    position: [ndc_left, ndc_top, 0.0],
                    tex_coords: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
            ];
            let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

            present_pass.set_bind_group(1, hud_bg, &[]);
            let vbuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("hud_vbuf"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ibuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("hud_ibuf"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            present_pass.set_vertex_buffer(0, vbuf.slice(..));
            present_pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint16);
            present_pass.draw_indexed(0..6, 0, 0..1);
        }

        drop(present_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    // Headless rendering method - renders to virtual canvas and returns pixel data for testing
    pub fn render_to_virtual_canvas(
        &mut self,
        engine_state: &crate::state::EngineState,
    ) -> Result<Vec<u8>> {
        // Ensure scene texture exists for current virtual mode
        self.ensure_scene_texture(engine_state)?;

        // Clear previous frame data
        self.sprite_vertices.clear();
        self.sprite_indices.clear();

        // Update transforms and build sprite batches from engine state
        self.update_from_engine_state(engine_state)?;

        // Upload sprite vertex/index data if it exists
        if !self.sprite_vertices.is_empty() {
            self.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&self.sprite_vertices),
            );
            self.queue.write_buffer(
                &self.index_buffer,
                0,
                bytemuck::cast_slice(&self.sprite_indices),
            );
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless_render_encoder"),
            });

        // Only do the virtual pass - render sprites to the virtual canvas (offscreen texture)
        if let Some(scene) = &self.scene_texture {
            let (vw, vh) = self.virtual_size;
            let projection_virtual =
                Mat4::orthographic_lh(0.0, vw as f32, 0.0, vh as f32, -1000.0, 1000.0);
            self.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&projection_virtual.to_cols_array()),
            );

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("virtual_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &scene.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear({
                            let cc = engine_state.get_clear_color();
                            wgpu::Color {
                                r: cc[0] as f64,
                                g: cc[1] as f64,
                                b: cc[2] as f64,
                                a: cc[3] as f64,
                            }
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if !self.sprite_vertices.is_empty() {
                pass.set_pipeline(&self.render_pipeline);
                pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

                let white_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.white_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.white_texture.sampler),
                        },
                    ],
                    label: Some("white_bg"),
                });

                for batch in &self.batches {
                    if let Some(bg) = self.get_bind_group(batch.texture_id) {
                        pass.set_bind_group(1, bg, &[]);
                    } else {
                        pass.set_bind_group(1, &white_bind_group, &[]);
                    }
                    pass.draw_indexed(
                        batch.start_index..(batch.start_index + batch.index_count),
                        0,
                        0..1,
                    );
                }
            }
            drop(pass);

            // Extract pixel data from virtual canvas texture
            let (vw, vh) = self.virtual_size;
            let bytes_per_pixel = 4u32;
            let output_size = (vw * vh * bytes_per_pixel) as u64;

            let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: output_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &scene.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &staging,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(vw * bytes_per_pixel),
                        rows_per_image: Some(vh),
                    },
                },
                wgpu::Extent3d {
                    width: vw,
                    height: vh,
                    depth_or_array_layers: 1,
                },
            );

            self.queue.submit(Some(encoder.finish()));

            let buffer_slice = staging.slice(..);
            buffer_slice.map_async(wgpu::MapMode::Read, move |_| {});
            self.device.poll(wgpu::Maintain::Wait);
            let data = buffer_slice.get_mapped_range().to_vec();
            let _ = buffer_slice;
            staging.unmap();
            Ok(data)
        } else {
            Err(anyhow::anyhow!("Scene texture not available"))
        }
    }

    pub fn update_from_engine_state(
        &mut self,
        engine_state: &crate::state::EngineState,
    ) -> Result<()> {
        // Sync clear color from engine state
        let cc = engine_state.get_clear_color();
        self.clear_color = wgpu::Color {
            r: cc[0] as f64,
            g: cc[1] as f64,
            b: cc[2] as f64,
            a: cc[3] as f64,
        };
        self.virtual_mode = engine_state.get_virtual_resolution();
        // Ensure scene texture exists for current virtual mode
        self.ensure_scene_texture(engine_state)?;
        // Clear previous frame data
        self.sprite_vertices.clear();
        self.sprite_indices.clear();

        // Update transforms
        self.set_transforms_v2(engine_state.get_transforms())?;

        // Sort sprites by z-coordinate first, then group consecutive sprites with same texture
        self.batches.clear();
        let sprites = engine_state.get_sprites();

        // Filter sprites that have transforms and sort by z-coordinate
        let mut sorted_sprites: Vec<&crate::state::SpriteData> = sprites
            .iter()
            .filter(|sd| self.transforms.contains_key(&sd.entity_id))
            .collect();
        sorted_sprites.sort_by(|a, b| a.z.partial_cmp(&b.z).unwrap_or(std::cmp::Ordering::Equal));

        let mut sprite_idx = 0usize;
        let mut current_batch_texture = None;
        let mut current_batch_start = 0u32;

        for sd in sorted_sprites.into_iter() {
            // Ensure texture and bind group cache
            if let Some(bytes) = engine_state.get_texture(sd.texture_id) {
                self.ensure_texture_cached(sd.texture_id, bytes)?;
            }

            if let Some(transform) = self.transforms.get(&sd.entity_id) {
                // Check if we need to start a new batch (different texture or first sprite)
                if current_batch_texture != Some(sd.texture_id) {
                    // Finish previous batch if it exists
                    if let Some(tex_id) = current_batch_texture {
                        let end = self.sprite_indices.len() as u32;
                        if end > current_batch_start {
                            self.batches.push(DrawBatch {
                                texture_id: tex_id,
                                start_index: current_batch_start,
                                index_count: end - current_batch_start,
                            });
                        }
                    }
                    // Start new batch
                    current_batch_texture = Some(sd.texture_id);
                    current_batch_start = self.sprite_indices.len() as u32;
                }

                let sprite_instance = SpriteInstance {
                    entity_id: sd.entity_id,
                    texture_id: sd.texture_id,
                    position: transform.position,
                    rotation: transform.rotation,
                    size: transform.size, // Use direct pixel size
                    uv_rect: Vec4::new(sd.uv[0], sd.uv[1], sd.uv[2], sd.uv[3]),
                    color: Vec4::new(sd.color[0], sd.color[1], sd.color[2], sd.color[3]),
                };
                self.add_sprite_to_batch(sprite_instance, sprite_idx)?;
                sprite_idx += 1;
            }
        }

        // Finish final batch if it exists
        if let Some(tex_id) = current_batch_texture {
            let end = self.sprite_indices.len() as u32;
            if end > current_batch_start {
                self.batches.push(DrawBatch {
                    texture_id: tex_id,
                    start_index: current_batch_start,
                    index_count: end - current_batch_start,
                });
            }
        }
        self.last_draw_calls = self.batches.len() as u32;

        Ok(())
    }

    fn ensure_scene_texture(&mut self, engine_state: &crate::state::EngineState) -> Result<()> {
        let (vw, vh) = match engine_state.get_virtual_resolution() {
            crate::state::VirtualResolution::Retro320x180 => (320u32, 180u32),
            crate::state::VirtualResolution::Hd1920x1080 => (1920u32, 1080u32),
        };

        let recreate = match &self.scene_texture {
            Some(existing) => {
                let (current_w, current_h) = (existing.texture.width(), existing.texture.height());
                current_w != vw || current_h != vh
            }
            None => true,
        };

        if recreate {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("scene_texture"),
                size: wgpu::Extent3d {
                    width: vw,
                    height: vh,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format, // Must match render pipeline format
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let scene_sampler = self
                .device
                .create_sampler(&wgpu::SamplerDescriptor::default());
            self.scene_texture = Some(Texture {
                texture,
                view,
                sampler: scene_sampler,
            });
            self.virtual_size = (vw, vh);

            // Update the bind group as well
            if let Some(scene) = &self.scene_texture {
                self.scene_bind_group =
                    Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &self.texture_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&scene.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.nearest_sampler),
                            },
                        ],
                        label: Some("scene_bind_group"),
                    }));
            }
        }
        Ok(())
    }

    pub fn get_sprite_count(&self) -> u32 {
        (self.sprite_vertices.len() / 4) as u32
    }

    pub fn get_draw_call_count(&self) -> u32 {
        self.last_draw_calls
    }

    fn ensure_texture_cached(&mut self, tex_id: u32, bytes: &[u8]) -> Result<()> {
        let slot = tex_id as usize;
        if slot >= self.textures.len() {
            self.textures.resize_with(slot + 1, || None);
            self.texture_bind_groups.resize_with(slot + 1, || None);
        }
        if self.textures[slot].is_none() {
            let tex =
                Texture::from_bytes(&self.device, &self.queue, bytes, &format!("tex_{}", tex_id))?;
            self.textures[slot] = Some(tex);
        }
        if self.texture_bind_groups[slot].is_none() {
            let t = self.textures[slot].as_ref().unwrap();
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&t.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&t.sampler),
                    },
                ],
                label: Some("sprite_texture_bind_group"),
            });
            self.texture_bind_groups[slot] = Some(bind_group);
        }
        Ok(())
    }

    fn get_bind_group(&self, tex_id: u32) -> Option<&wgpu::BindGroup> {
        let slot = tex_id as usize;
        if slot < self.texture_bind_groups.len() {
            self.texture_bind_groups[slot].as_ref()
        } else {
            None
        }
    }

    pub fn set_hud_rgba(&mut self, rgba: &[u8], w: u32, h: u32) -> Result<()> {
        // Create texture and bind group for HUD panel
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hud_texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("hud_bind_group"),
        });
        self.hud_texture = Some(Texture {
            texture,
            view,
            sampler,
        });
        self.hud_bind_group = Some(bind_group);
        self.hud_size = (w, h);
        Ok(())
    }

    pub fn set_hud_scale(&mut self, scale: f32) {
        self.hud_scale = scale.max(1.0);
    }
}

#[derive(Debug, Clone, Copy)]
struct DrawBatch {
    texture_id: u32,
    start_index: u32,
    index_count: u32,
}
