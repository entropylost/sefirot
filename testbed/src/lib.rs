use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::time::Instant;

use agx::AgXParameters;
use keter::lang::types::vector::{Vec2, Vec3, Vec4};
use keter::prelude::*;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalSize, Size};
pub use winit::event::MouseButton;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
pub use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

pub mod agx;

pub struct Runtime {
    swapchain: Swapchain,
    display_texture: Tex2d<Vec4<f32>>,
    staging_texture: Tex2d<Vec3<f32>>,
    overlay_texture: Tex2d<Vec4<f32>>,
    tonemap_display: Kernel<fn(Tex2d<Vec4<f32>>)>,
    pub mouse_scroll: Vec2<f32>,
    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub pressed_buttons: HashSet<MouseButton>,
    pub just_pressed_buttons: HashSet<MouseButton>,
    pub cursor_position: Vec2<f32>,
    last_cursor_position: Vec2<f32>,
    pub tick: u32,
    pub average_frame_time: f64,
    last_frame_start_time: Instant,
    last_frame_time: f64,
    pub scale: u32,
    resize_time: Option<Instant>,
    resize: bool,
    grid_size: [u32; 2],
    #[cfg(feature = "video")]
    pub encoder: Option<(video_rs::Encoder, video_rs::Time)>,
}

impl Runtime {
    pub fn fps(&self) -> f32 {
        1.0 / self.average_frame_time as f32
    }
    pub fn log_fps(&self) {
        if self.tick % 60 == 0 {
            println!("FPS: {:.2}", self.fps());
        }
    }
    // Time of the last frame in seconds.
    pub fn frame_time(&self) -> f64 {
        self.last_frame_time
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
    pub fn overlay(&self) -> &Tex2d<Vec4<f32>> {
        &self.overlay_texture
    }
    pub fn cursor_velocity(&self) -> Vec2<f32> {
        if self.last_cursor_position == Vec2::splat(f32::NEG_INFINITY)
            || self.cursor_position == Vec2::splat(f32::NEG_INFINITY)
        {
            Vec2::splat(0.0)
        } else {
            self.cursor_position - self.last_cursor_position
        }
    }
    pub fn width(&self) -> u32 {
        self.grid_size[0]
    }
    pub fn height(&self) -> u32 {
        self.grid_size[1]
    }
    pub fn size(&self) -> [u32; 2] {
        self.grid_size
    }
    pub fn dispatch_size(&self) -> [u32; 3] {
        [self.grid_size[0], self.grid_size[1], 1]
    }
    pub fn display_size(&self) -> [u32; 2] {
        [
            self.grid_size[0] * self.scale,
            self.grid_size[1] * self.scale,
        ]
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

struct RunningApp<F: FnMut(&mut Runtime, Scope)> {
    runtime: Runtime,
    window: Window,
    update_fn: F,
}
impl<F: FnMut(&mut Runtime, Scope)> ApplicationHandler for RunningApp<F> {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = &self.window;
        if window_id != window.id() {
            return;
        }
        let runtime = &mut self.runtime;
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::CursorLeft { .. } => {
                runtime.cursor_position = Vec2::splat(f32::NEG_INFINITY);
            }
            WindowEvent::CursorMoved { position, .. } => {
                runtime.last_cursor_position = runtime.cursor_position;
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
            WindowEvent::MouseWheel { delta, .. } => match delta {
                winit::event::MouseScrollDelta::LineDelta(x, y) => {
                    runtime.mouse_scroll.x += x;
                    runtime.mouse_scroll.y += y;
                }
                winit::event::MouseScrollDelta::PixelDelta(position) => {
                    runtime.mouse_scroll.x += position.x as f32;
                    runtime.mouse_scroll.y += position.y as f32;
                }
            },

            WindowEvent::Resized(size) => {
                let display_size = runtime.display_size();
                if runtime.resize {
                    runtime.resize_time = Some(Instant::now());
                } else if size.width != display_size[0] || size.height != display_size[1] {
                    let _ = window
                        .request_inner_size(PhysicalSize::new(display_size[0], display_size[1]));
                }
            }
            WindowEvent::RedrawRequested => {
                window.request_redraw();

                let scope = DEVICE.default_stream().scope();
                scope.submit([runtime.tonemap_display.dispatch_async(
                    [runtime.grid_size[0], runtime.grid_size[1], 1],
                    &runtime.display_texture,
                )]);
                scope.present(&runtime.swapchain, &runtime.display_texture);
                let start = Instant::now();
                runtime.last_frame_time = (start - runtime.last_frame_start_time).as_secs_f64();
                runtime.last_frame_start_time = start;
                (self.update_fn)(runtime, scope);
                let delta = start.elapsed().as_secs_f64();
                runtime.average_frame_time = runtime.average_frame_time * 0.99 + delta * 0.01;
                runtime.last_frame_time = delta;
                runtime.tick += 1;

                if runtime
                    .resize_time
                    .is_some_and(|t| t.elapsed().as_secs_f32() > 0.1)
                {
                    runtime.resize_time = None;
                    let size = window.inner_size();
                    let swapchain = DEVICE.create_swapchain(
                        window,
                        &DEVICE.default_stream(),
                        size.width,
                        size.height,
                        false,
                        false,
                        3,
                    );
                    let display_texture = DEVICE.create_tex2d::<Vec4<f32>>(
                        swapchain.pixel_storage(),
                        size.width,
                        size.height,
                        1,
                    );
                    runtime.swapchain = swapchain;
                    runtime.display_texture = display_texture;
                    runtime.grid_size = [size.width / runtime.scale, size.height / runtime.scale];
                    println!("Resized to {:?}", runtime.grid_size);
                }

                runtime.just_pressed_keys.clear();
                runtime.just_pressed_buttons.clear();
                runtime.mouse_scroll = Vec2::splat(0.0);

                #[cfg(feature = "video")]
                if let Some((encoder, position)) = &mut runtime.encoder {
                    let frame: Vec<Vec4<u8>> = runtime.display_texture.view(0).copy_to_vec();

                    let frame_array = ndarray::Array3::from_shape_fn(
                        (
                            runtime.display_texture.width() as usize,
                            runtime.display_texture.height() as usize,
                            3,
                        ),
                        |(x, y, c)| {
                            (<[u8; 4]>::from(
                                frame[x * runtime.display_texture.height() as usize + y],
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
                    event_loop.exit();
                }
            }
            _ => (),
        }
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
    pub fn run(self, update: impl FnMut(&mut Runtime, Scope)) {
        self.event_loop.set_control_flow(ControlFlow::Poll);
        self.event_loop
            .run_app(&mut RunningApp {
                runtime: self.runtime,
                window: self.window,
                update_fn: update,
            })
            .unwrap();
    }
    #[allow(clippy::new_ret_no_self)]
    pub fn new(name: impl Into<String>, grid_size: [u32; 2]) -> AppBuilder {
        AppBuilder {
            name: name.into(),
            grid_size,
            scale: 1,
            agx: None,
            gamma: 2.2,
            resize: false,
        }
    }
}

pub struct AppBuilder {
    pub name: String,
    pub grid_size: [u32; 2],
    pub scale: u32,
    pub agx: Option<Option<AgXParameters>>,
    pub gamma: f32,
    pub resize: bool,
}
impl AppBuilder {
    pub fn scale(mut self, scale: u32) -> Self {
        self.scale = scale;
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
    pub fn gamma(mut self, gamma: f32) -> Self {
        self.gamma = gamma;
        self
    }
    pub fn resize(mut self) -> Self {
        self.resize = true;
        self
    }
    pub fn finish(self) -> App {
        self.init()
    }
    pub fn init(self) -> App {
        keter::init_logger();

        #[cfg(feature = "video")]
        video_rs::init().unwrap();

        let AppBuilder {
            name,
            grid_size,
            scale,
            agx,
            gamma,
            resize,
        } = self;

        let w = grid_size[0] * scale;
        let h = grid_size[1] * scale;

        let event_loop = EventLoop::new().unwrap();
        let window = Window::default_attributes()
            .with_title(name)
            .with_resizable(resize)
            .with_inner_size(PhysicalSize::new(w, h))
            .with_resize_increments(Size::Physical(PhysicalSize::new(scale, scale)));
        #[expect(deprecated)]
        // Safe since we aren't on android.
        // https://docs.rs/winit/latest/winit/application/trait.ApplicationHandler.html
        let window = event_loop.create_window(window).unwrap();

        let mut max_w = w;
        let mut max_h = h;
        let mut dpi = 1.0_f64;
        for monitor in window.available_monitors() {
            let size = monitor.size();
            max_w = max_w.max(size.width);
            max_h = max_h.max(size.height);
            dpi = dpi.max(monitor.scale_factor());
        }
        let dpi_diff = dpi / window.scale_factor();
        let _ =
            window.request_inner_size(PhysicalSize::new(w / dpi_diff as u32, h / dpi_diff as u32));

        let swapchain =
            DEVICE.create_swapchain(&window, &DEVICE.default_stream(), w, h, false, false, 3);

        let display_texture = DEVICE.create_tex2d::<Vec4<f32>>(swapchain.pixel_storage(), w, h, 1);
        let overlay_texture =
            DEVICE.create_tex2d::<Vec4<f32>>(PixelStorage::Float4, max_w, max_h, 1);
        let staging_texture = DEVICE.create_tex2d::<Vec3<f32>>(
            PixelStorage::Float4,
            max_w.div_ceil(scale),
            max_h.div_ceil(scale),
            1,
        );

        let tonemap_display =
            DEVICE.create_kernel_async::<fn(Tex2d<Vec4<f32>>)>(&track!(|display_texture| {
                let value = staging_texture.read(dispatch_id().xy());
                let value = if let Some(params) = agx {
                    agx::agx_tonemap(value, params)
                } else {
                    value.powf(1.0 / gamma)
                };
                for i in 0..scale {
                    for j in 0..scale {
                        let pos = dispatch_id().xy() * scale + Vec2::expr(i, j);
                        let overlay = overlay_texture.read(pos);
                        display_texture
                            .write(pos, value.lerp(overlay.xyz(), overlay.w).extend(1.0));
                        overlay_texture.write(pos, Vec4::splat(0.0));
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
                overlay_texture,
                tonemap_display,
                pressed_keys: HashSet::new(),
                just_pressed_keys: HashSet::new(),
                pressed_buttons: HashSet::new(),
                just_pressed_buttons: HashSet::new(),
                cursor_position: Vec2::splat(f32::NEG_INFINITY),
                last_cursor_position: Vec2::splat(f32::NEG_INFINITY),
                mouse_scroll: Vec2::splat(0.0),
                tick: 0,
                average_frame_time: 0.016,
                last_frame_start_time: Instant::now(),
                last_frame_time: 0.016,
                scale,
                grid_size,
                resize_time: None,
                resize,
                #[cfg(feature = "video")]
                encoder: None,
            },
        }
    }
}

pub fn init(name: impl Into<String>, grid_size: [u32; 2], scale: u32) -> App {
    App::new(name, grid_size).scale(scale).init()
}
