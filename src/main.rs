const CIRCLES: usize = 3000;

const NUM_COLORS: u32 = 6;
const COLORS: [[u8; 4]; NUM_COLORS as usize] = [
    [255, 0, 0, 255],   // RED
    [255, 128, 0, 255],   // ORANGE
    [255, 255, 0, 255],   // YELLOW
    [0, 255, 0, 255],   // GREEN
    [0, 0, 255, 255],   // BLUE
    [255, 0, 255, 255],   // PURPLE

];
/*const CONSTRAINTS: [[[f32; 4]; NUM_COLORS as usize]; NUM_COLORS as usize] = [
    [[1.0, 0.0, 0.0, 0.0], [0.2, 0.0, 0.0, 0.0], [-0.2, 0.0, 0.0, 0.0]],
    [[-0.2, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [0.2, 0.0, 0.0, 0.0]],
    [[0.2, 0.0, 0.0, 0.0], [-0.2, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]]
];*/

const ZOOM: f32 = 20.0;
const CAMERA_MOVE_SPEED: f32 = 20.0;
const CAMERA_ZOOM_SPEED: f32 = 2.0;

mod camera;
mod circle;

use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
    window::WindowBuilder,
};
use wgpu::util::DeviceExt;

use rand::random;
use std::time::Instant;

use camera::Camera;
use circle::Circle;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pub position: [f32; 2],
}

const SQUARE_SHAPE: &[Vertex] = &[
    Vertex { position: [-1.0, -1.0], },
    Vertex { position: [ 1.0, -1.0], },
    Vertex { position: [ 1.0,  1.0], },
    Vertex { position: [ 1.0,  1.0], },
    Vertex { position: [-1.0,  1.0], },
    Vertex { position: [-1.0, -1.0], },
];

struct State {
    pause: bool,

    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    
    camera: Camera,
    last_frame: Instant,
    camera_buffer: wgpu::Buffer,
    size_buffer: wgpu::Buffer,
    render_uniform_bind_group: wgpu::BindGroup,

    dt_buffer: wgpu::Buffer,
    compute_uniform_bind_group: wgpu::BindGroup,
    circ_buffer: wgpu::Buffer,
    constraints_tex: wgpu::Texture,
    circ_bind_group: wgpu::BindGroup,

    compute_pipeline: wgpu::ComputePipeline,

    keys: [bool; 256],
}

fn main() {
    // set up context and build window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut state = pollster::block_on(State::new(window));

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() => match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            WindowEvent::KeyboardInput { input, .. } => match input.virtual_keycode {
                Some(VirtualKeyCode::Escape) => *control_flow = ControlFlow::Exit,
                Some(VirtualKeyCode::Space) if matches!(input.state, ElementState::Pressed) => state.pause = !state.pause,
                Some(VirtualKeyCode::R) if matches!(input.state, ElementState::Pressed) => state.randomize_constraints(),
                Some(k) => state.keys[k as usize] = match input.state {
                    ElementState::Pressed => true,
                    ElementState::Released => false,
                },
                _ => {}
            },
            WindowEvent::Resized(physical_size) => {
                state.resize(*physical_size);
            }
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                state.resize(**new_inner_size);
            }
            w => state.input(w),
        },
        Event::RedrawRequested(window_id) if window_id == state.window().id() => {
            state.update();
            match state.render() {
                Ok(_) => {}
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                // The system is out of memory, we should probably quit
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                // All other errors (Outdated, Timeout) should be resolved by the next frame
                Err(e) => eprintln!("{:?}", e),
            }
        }
        Event::MainEventsCleared => {
            state.window().request_redraw();
        }
        _ => {}
    });
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(window: Window) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::VERTEX_WRITABLE_STORAGE,
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],//wgpu::PresentMode::AutoVsync, //surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let camera = Camera {
            pos: [0.0, 0.0],
            scale: 1.0 / ZOOM,
        };

        let mut circles = Vec::with_capacity(CIRCLES);
        for _ in 0..CIRCLES {
            circles.push(Circle {
                pos: [
                    (random::<f32>() - 0.5) * 2.0 * ZOOM,
                    (random::<f32>() - 0.5) * 2.0 * ZOOM,
                ],
                vel: [(random::<f32>() - 0.5) * 2.0, (random::<f32>() - 0.5) * 5.0],
                rad: 0.125,
                color: (random::<f32>() * NUM_COLORS as f32) as i32,
            });
        }

        let mut constraints: [[[f32; 4]; NUM_COLORS as usize]; NUM_COLORS as usize] = std::default::Default::default();
        for i in 0..NUM_COLORS as usize {
            let mut attractions: [[f32; 4]; NUM_COLORS as usize] = std::default::Default::default();
            for j in 0..NUM_COLORS as usize {
                attractions[j] = [(random::<f32>() - 0.5), 0.0, 0.0, 0.0];
            }
            constraints[i] = attractions;
        }
        //let constraints = CONSTRAINTS;

        let camera_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&camera.transform()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let size_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Size Buffer"),
                contents: bytemuck::cast_slice(&[size.width, size.height]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let dt_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("dt buffer"),
                contents: bytemuck::cast_slice(&[0.0f32]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let colors_tex_size = wgpu::Extent3d { 
            width: NUM_COLORS,
            height: 1,
            depth_or_array_layers: 1,
        };
        let colors_tex = device.create_texture(
            &wgpu::TextureDescriptor {
                label: Some("Colors buffer"),
                size: colors_tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D1,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &colors_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            }, 
            bytemuck::cast_slice(&COLORS), 
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * NUM_COLORS),
                rows_per_image: Some(1),
            }, 
            colors_tex_size,
        );
        let colors_tex_view = colors_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let constraints_tex_size = wgpu::Extent3d { 
            width: NUM_COLORS,
            height: NUM_COLORS,
            depth_or_array_layers: 1,
        };
        let constraints_tex = device.create_texture(
            &wgpu::TextureDescriptor {
                label: Some("Constraints buffer"),
                size: constraints_tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &constraints_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            }, 
            bytemuck::cast_slice(&constraints), 
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(16 * NUM_COLORS),
                rows_per_image: Some(NUM_COLORS),
            }, 
            constraints_tex_size,
        );
        let constraints_tex_view = constraints_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let data_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            .. Default::default()
        });

        let circ_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Circle Buffer"),
                contents: bytemuck::cast_slice(&circles),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            }
        );

        let render_uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
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
                        view_dimension: wgpu::TextureViewDimension::D1, 
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
            label: Some("uniform_bind_group_layout"),
        });

        let compute_uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { 
                        sample_type: wgpu::TextureSampleType::Float { filterable: false }, 
                        view_dimension: wgpu::TextureViewDimension::D2, 
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { 
                        sample_type: wgpu::TextureSampleType::Float { filterable: true }, 
                        view_dimension: wgpu::TextureViewDimension::D1, 
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
            label: Some("compute uniform bind group layout"),
        });

        let circ_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { 
                        ty: wgpu::BufferBindingType::Storage { read_only: false }, 
                        has_dynamic_offset: false, 
                        min_binding_size: None, 
                    },
                    count: None,
                }
            ],
            label: Some("circle bind group layout"),
        });

        let render_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &render_uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: size_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&colors_tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&data_sampler),
                },
            ],
            label: Some("uniform_bind_group"),
        });

        let circ_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &circ_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: circ_buffer.as_entire_binding(),
                }
            ],
            label: Some("circ bind group"),
        });

        let compute_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute uniform bind group"),
            layout: &compute_uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: dt_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&constraints_tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&colors_tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&data_sampler),
                },
            ]
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&render_uniform_bind_group_layout, &circ_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    Vertex::desc(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false, 
            },
            multiview: None, 
        });

        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(SQUARE_SHAPE),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );

        let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute pipeline descriptinator"),
            bind_group_layouts: &[
                &compute_uniform_bind_group_layout,
                &circ_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: &"compute_main",
        });

        Self {
            pause: true,

            window,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            
            camera,
            last_frame: Instant::now(),
            camera_buffer,
            size_buffer,
            render_uniform_bind_group,
            
            dt_buffer,
            compute_uniform_bind_group,
            circ_buffer,
            constraints_tex,
            circ_bind_group,
            compute_pipeline,

            keys: [false; 256],
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self, event: &WindowEvent) {}

    fn update(&mut self) {
        let dt = f32::min(0.005, self.last_frame.elapsed().as_secs_f32());

        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&self.camera.transform()));
        self.queue.write_buffer(&self.size_buffer, 0, bytemuck::cast_slice(&[self.size.width, self.size.height]));
        self.queue.write_buffer(&self.dt_buffer, 0, bytemuck::cast_slice(&[dt]));

        if self.keys[VirtualKeyCode::W as usize] { self.camera.pos[1] -= CAMERA_MOVE_SPEED * dt}
        if self.keys[VirtualKeyCode::A as usize] { self.camera.pos[0] += CAMERA_MOVE_SPEED * dt}
        if self.keys[VirtualKeyCode::S as usize] { self.camera.pos[1] += CAMERA_MOVE_SPEED * dt}
        if self.keys[VirtualKeyCode::D as usize] { self.camera.pos[0] -= CAMERA_MOVE_SPEED * dt}
        if self.keys[VirtualKeyCode::Up as usize] { self.camera.scale *= 1.0 + CAMERA_ZOOM_SPEED * dt}
        if self.keys[VirtualKeyCode::Down as usize] { self.camera.scale *= 1.0 - CAMERA_ZOOM_SPEED * dt}
        self.camera.scale = self.camera.scale.clamp(0.0, 1.0);

        //println!("{}", self.last_frame.elapsed().as_secs_f32().recip());
        self.last_frame = Instant::now();

        if self.pause { return; }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute encoder"),
        });
        
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute pass"),
            });
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_uniform_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.circ_bind_group, &[]);
            compute_pass.dispatch_workgroups(CIRCLES as u32, 1, 1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        //wgpu::util::DownloadBuffer::read_buffer(&self.device, &self.queue, &self.circ_buffer.slice(..), |r| {if let Ok(buf) = r {println!("{:?}", bytemuck::from_bytes::<[Circle; CIRCLES]>(&buf));}});
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.render_uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.circ_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..CIRCLES as u32);
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn randomize_constraints(&mut self) {
        println!("r pressed");
        let mut constraints: [[[f32; 4]; NUM_COLORS as usize]; NUM_COLORS as usize] = std::default::Default::default();
        for i in 0..NUM_COLORS as usize {
            let mut attractions: [[f32; 4]; NUM_COLORS as usize] = std::default::Default::default();
            for j in 0..NUM_COLORS as usize {
                attractions[j] = [(random::<f32>() - 0.5) * 2.0, 0.0, 0.0, 0.0];
            }
            constraints[i] = attractions;
        }

        let constraints_tex_size = wgpu::Extent3d { 
            width: NUM_COLORS,
            height: NUM_COLORS,
            depth_or_array_layers: 1,
        };
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.constraints_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            }, 
            bytemuck::cast_slice(&constraints), 
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(16 * NUM_COLORS),
                rows_per_image: Some(NUM_COLORS),
            }, 
            constraints_tex_size,
        );
    }
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ]
        }
    }
}