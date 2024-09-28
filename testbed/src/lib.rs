use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::time::Instant;

use agx::AgXParameters;
use luisa_compute::lang::types::vector::{Vec2, Vec3, Vec4};
use sefirot::prelude::*;
use winit::dpi::{LogicalSize, PhysicalSize, Size};
pub use winit::event::MouseButton;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
pub use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::Window;

pub mod agx;

pub struct Runtime {
    swapchain: Swapchain,
    display_texture: Tex2d<Vec4<f32>>,
    staging_texture: Tex2d<Vec3<f32>>,
    tonemap_display: luisa::runtime::Kernel<fn()>,
    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub pressed_buttons: HashSet<MouseButton>,
    pub just_pressed_buttons: HashSet<MouseButton>,
    pub cursor_position: Vec2<f32>,
    pub tick: u32,
    pub average_frame_time: f32,
    pub scale: u32,
    #[cfg(feature = "video")]
    pub encoder: Option<(video_rs::Encoder, video_rs::Time)>,
}

impl Runtime {
    pub fn fps(&self) -> f32 {
        1.0 / self.average_frame_time
    }
    pub fn log_fps(&self) {
        if self.tick % 60 == 0 {
            println!("FPS: {:.2}", self.fps());
        }
    }
    pub fn pressed_key(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }
    pub fn just_pressed_key(&self, key: KeyCode) -> bool {
        self.just_pressed_keys.contains(&key)
    }
    pub fn pressed_button(&self, button: MouseButton) -> bool {
        self.pressed_buttons.contains(&button)
    }
    pub fn just_pressed_button(&self, button: MouseButton) -> bool {
        self.just_pressed_buttons.contains(&button)
    }
    pub fn final_display(&self) -> &Tex2d<Vec4<f32>> {
        &self.display_texture
    }
    pub fn display(&self) -> &Tex2d<Vec3<f32>> {
        &self.staging_texture
    }

    #[cfg(feature = "video")]
    pub fn finish_recording(&mut self) {
        if let Some((mut encoder, _)) = self.encoder.take() {
            encoder.finish().unwrap();
        } else {
            eprintln!("Warning: Haven't started recording yet.");
        }
    }

    #[cfg(feature = "video")]
    pub fn begin_recording(&mut self, path: Option<&str>, realtime: bool) {
        if self.encoder.is_some() {
            self.finish_recording();
        }
        let settings = video_rs::encode::Settings::preset_h264_yuv420p(
            self.display_texture.width() as usize,
            self.display_texture.height() as usize,
            realtime,
        );
        let path = path.map_or_else(
            || {
                std::path::Path::new(&format!(
                    "recording-{}.mp4",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("Temporal anomaly detected.")
                        .as_millis()
                ))
                .to_path_buf()
            },
            |p| std::path::PathBuf::from(p),
        );
        let encoder = video_rs::Encoder::new(path, settings).unwrap();
        self.encoder = Some((encoder, video_rs::Time::zero()));
    }
}

pub struct App {
    event_loop: EventLoop<()>,
    window: Window,
    pub runtime: Runtime,
}

impl Deref for App {
    type Target = Runtime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}
impl DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}

impl App {
    pub fn run(mut self, mut update: impl FnMut(&mut Runtime, Scope)) {
        let runtime = &mut self.runtime;
        self.event_loop.set_control_flow(ControlFlow::Poll);
        self.event_loop
            .run(move |event, elwt| match event {
                Event::WindowEvent { event, window_id } if window_id == self.window.id() => {
                    match event {
                        WindowEvent::CloseRequested => {
                            elwt.exit();
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            runtime.cursor_position = Vec2::new(
                                position.x as f32 / runtime.scale as f32,
                                position.y as f32 / runtime.scale as f32,
                            );
                        }
                        WindowEvent::MouseInput { button, state, .. } => match state {
                            ElementState::Pressed => {
                                runtime.pressed_buttons.insert(button);
                                runtime.just_pressed_buttons.insert(button);
                            }
                            ElementState::Released => {
                                runtime.pressed_buttons.remove(&button);
                            }
                        },
                        WindowEvent::KeyboardInput { event, .. } => {
                            let PhysicalKey::Code(key) = event.physical_key else {
                                return;
                            };
                            match event.state {
                                ElementState::Pressed => {
                                    runtime.pressed_keys.insert(key);
                                    runtime.just_pressed_keys.insert(key);
                                }
                                ElementState::Released => {
                                    runtime.pressed_keys.remove(&key);
                                }
                            }
                        }
                        WindowEvent::RedrawRequested => {
                            self.window.request_redraw();
                            let scope = DEVICE.default_stream().scope();
                            scope.submit([runtime.tonemap_display.dispatch_async([
                                runtime.staging_texture.width(),
                                runtime.staging_texture.height(),
                                1,
                            ])]);
                            scope.present(&runtime.swapchain, &runtime.display_texture);
                            let start = Instant::now();
                            update(runtime, scope);
                            let delta = start.elapsed().as_secs_f32();
                            runtime.average_frame_time =
                                runtime.average_frame_time * 0.99 + delta * 0.01;
                            runtime.tick += 1;

                            runtime.just_pressed_keys.clear();
                            runtime.just_pressed_buttons.clear();

                            #[cfg(feature = "video")]
                            if let Some((encoder, position)) = &mut runtime.encoder {
                                let frame: Vec<Vec4<u8>> =
                                    runtime.display_texture.view(0).copy_to_vec();

                                let frame_array = ndarray::Array3::from_shape_fn(
                                    (
                                        runtime.display_texture.width() as usize,
                                        runtime.display_texture.height() as usize,
                                        3,
                                    ),
                                    |(x, y, c)| {
                                        (<[u8; 4]>::from(
                                            frame
                                                [x * runtime.display_texture.height() as usize + y],
                                        ))[c]
                                    },
                                );

                                encoder.encode(&frame_array, *position).unwrap();
                                *position = position
                                    .aligned_with(video_rs::Time::from_nth_of_a_second(60))
                                    .add();
                            }

                            if runtime.pressed_key(KeyCode::Escape) {
                                #[cfg(feature = "video")]
                                runtime.finish_recording();
                                elwt.exit();
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            })
            .unwrap();
    }
    #[allow(clippy::new_ret_no_self)]
    pub fn new(name: impl Into<String>, grid_size: [u32; 2]) -> AppBuilder {
        AppBuilder {
            name: name.into(),
            grid_size,
            scale: 1,
            dpi_override: None,
            agx: None,
        }
    }
}

pub struct AppBuilder {
    pub name: String,
    pub grid_size: [u32; 2],
    pub scale: u32,
    pub dpi_override: Option<f64>,
    pub agx: Option<Option<AgXParameters>>,
}
impl AppBuilder {
    pub fn scale(mut self, scale: u32) -> Self {
        self.scale = scale;
        self
    }
    pub fn dpi_override(mut self, dpi_override: f64) -> Self {
        self.dpi_override = Some(dpi_override);
        self
    }
    pub fn agx(mut self) -> Self {
        self.agx = Some(None);
        self
    }
    pub fn agx_params(mut self, params: AgXParameters) -> Self {
        self.agx = Some(Some(params));
        self
    }
    pub fn finish(self) -> App {
        self.init()
    }
    pub fn init(self) -> App {
        #[cfg(feature = "video")]
        video_rs::init().unwrap();

        let AppBuilder {
            name,
            grid_size,
            scale,
            dpi_override,
            agx,
        } = self;

        let w = grid_size[0] * scale;
        let h = grid_size[1] * scale;

        let event_loop = EventLoop::new().unwrap();
        let window = winit::window::WindowBuilder::new()
            .with_title(name)
            .with_inner_size::<Size>(if let Some(dpi) = dpi_override {
                LogicalSize::new(w as f64 / dpi, h as f64 / dpi).into()
            } else {
                PhysicalSize::new(w, h).into()
            })
            .with_resizable(false)
            .build(&event_loop)
            .unwrap();

        let swapchain =
            DEVICE.create_swapchain(&window, &DEVICE.default_stream(), w, h, false, false, 3);

        let display_texture = DEVICE.create_tex2d::<Vec4<f32>>(swapchain.pixel_storage(), w, h, 1);
        let staging_texture = DEVICE.create_tex2d::<Vec3<f32>>(PixelStorage::Float4, w, h, 1);

        let tonemap_display = DEVICE.create_kernel_async::<fn()>(&track!(|| {
            let value = staging_texture.read(dispatch_id().xy());
            let value = if let Some(params) = agx {
                agx::agx_tonemap(value, params)
            } else {
                value.powf(2.2_f32)
            };
            let value = value.extend(1.0);
            for i in 0..scale {
                for j in 0..scale {
                    display_texture.write(dispatch_id().xy() * scale + Vec2::expr(i, j), value);
                }
            }
            staging_texture.write(dispatch_id().xy(), Vec3::splat(0.0));
        }));

        App {
            event_loop,
            window,
            runtime: Runtime {
                swapchain,
                display_texture,
                staging_texture,
                tonemap_display,
                pressed_keys: HashSet::new(),
                just_pressed_keys: HashSet::new(),
                pressed_buttons: HashSet::new(),
                just_pressed_buttons: HashSet::new(),
                cursor_position: Vec2::new(0.0, 0.0),
                tick: 0,
                average_frame_time: 0.016,
                scale,
                #[cfg(feature = "video")]
                encoder: None,
            },
        }
    }
}

pub fn init(name: impl Into<String>, grid_size: [u32; 2], scale: u32) -> App {
    App::new(name, grid_size).scale(scale).init()
}
