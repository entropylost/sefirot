use std::collections::HashSet;
use std::env::current_exe;
use std::time::Instant;

use luisa_compute::lang::types::vector::{Vec2, Vec3, Vec4};
use luisa_compute::DeviceType;
use sefirot::prelude::*;
use winit::dpi::PhysicalSize;
pub use winit::event::MouseButton;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
pub use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::Window;

pub struct Runtime {
    device: Device,
    swapchain: Swapchain,
    display_texture: Tex2d<Vec4<f32>>,
    staging_texture: Tex2d<Vec3<f32>>,
    tonemap_display: luisa::runtime::Kernel<fn()>,
    pub pressed_keys: HashSet<KeyCode>,
    pub pressed_buttons: HashSet<MouseButton>,
    pub cursor_position: Vec2<f32>,
    pub tick: u32,
    pub average_frame_time: f32,
    pub scale: u32,
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
    pub fn pressed_button(&self, button: MouseButton) -> bool {
        self.pressed_buttons.contains(&button)
    }
}

pub struct App {
    event_loop: EventLoop<()>,
    window: Window,
    runtime: Runtime,
}

impl App {
    pub fn display(&self) -> &Tex2d<Vec3<f32>> {
        &self.runtime.staging_texture
    }
    pub fn run(mut self, mut update: impl FnMut(&Runtime, Scope)) {
        self.event_loop.set_control_flow(ControlFlow::Poll);
        self.event_loop
            .run(move |event, elwt| match event {
                Event::WindowEvent { event, window_id } if window_id == self.window.id() => {
                    match event {
                        WindowEvent::CloseRequested => {
                            elwt.exit();
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            self.runtime.cursor_position = Vec2::new(
                                position.x as f32 / self.runtime.scale as f32,
                                position.y as f32 / self.runtime.scale as f32,
                            );
                        }
                        WindowEvent::MouseInput { button, state, .. } => match state {
                            ElementState::Pressed => {
                                self.runtime.pressed_buttons.insert(button);
                            }
                            ElementState::Released => {
                                self.runtime.pressed_buttons.remove(&button);
                            }
                        },
                        WindowEvent::KeyboardInput { event, .. } => {
                            let PhysicalKey::Code(key) = event.physical_key else {
                                return;
                            };
                            match event.state {
                                ElementState::Pressed => {
                                    self.runtime.pressed_keys.insert(key);
                                }
                                ElementState::Released => {
                                    self.runtime.pressed_keys.remove(&key);
                                }
                            }
                        }
                        WindowEvent::RedrawRequested => {
                            self.window.request_redraw();
                        }
                        _ => (),
                    }
                }
                Event::AboutToWait => {
                    let scope = self.runtime.device.default_stream().scope();
                    scope.submit([self.runtime.tonemap_display.dispatch_async([
                        self.runtime.staging_texture.width(),
                        self.runtime.staging_texture.height(),
                        1,
                    ])]);
                    scope.present(&self.runtime.swapchain, &self.runtime.display_texture);
                    let start = Instant::now();
                    update(&self.runtime, scope);
                    let delta = start.elapsed().as_secs_f32();
                    self.runtime.average_frame_time =
                        self.runtime.average_frame_time * 0.99 + delta * 0.01;
                    self.window.request_redraw();
                    self.runtime.tick += 1;
                }
                _ => (),
            })
            .unwrap();
    }
}

pub fn init(name: impl Into<String>, grid_size: [u32; 2], scale: u32) -> (App, Device) {
    let w = grid_size[0] * scale;
    let h = grid_size[1] * scale;

    luisa::init_logger();

    let ctx = Context::new(current_exe().unwrap());
    let device = ctx.create_device(DeviceType::Cuda);

    let event_loop = EventLoop::new().unwrap();
    let window = winit::window::WindowBuilder::new()
        .with_title(name)
        .with_inner_size(PhysicalSize::new(w, h))
        .with_resizable(false)
        .build(&event_loop)
        .unwrap();

    let swapchain =
        device.create_swapchain(&window, &device.default_stream(), w, h, false, false, 3);

    let display_texture = device.create_tex2d::<Vec4<f32>>(swapchain.pixel_storage(), w, h, 1);
    let staging_texture = device.create_tex2d::<Vec3<f32>>(PixelStorage::Float4, w, h, 1);

    let tonemap_display = device.create_kernel_async::<fn()>(&track!(|| {
        let value = staging_texture.read(dispatch_id().xy());
        let value = value.powf(2.2_f32).extend(1.0_f32);
        for i in 0..scale {
            for j in 0..scale {
                display_texture.write(dispatch_id().xy() * scale + Vec2::expr(i, j), value);
            }
        }
    }));

    (
        App {
            event_loop,
            window,
            runtime: Runtime {
                swapchain,
                device: device.clone(),
                display_texture,
                staging_texture,
                tonemap_display,
                pressed_keys: HashSet::new(),
                pressed_buttons: HashSet::new(),
                cursor_position: Vec2::new(0.0, 0.0),
                tick: 0,
                average_frame_time: 0.016,
                scale,
            },
        },
        device,
    )
}
