use std::env::current_exe;

use luisa::lang::types::vector::{Vec2, Vec3, Vec4};
use sefirot::graph::ComputeGraph;
use sefirot::prelude::*;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Particles {}

impl EmanationType for Particles {}

#[derive(Debug, Clone, Copy, PartialEq, Value, Structure)]
#[repr(C)]
struct Particle {
    position: Vec2<f32>,
    velocity: Vec2<f32>,
}

const SIZE: u32 = 512;

fn main() {
    luisa::init_logger();
    let device = Context::new(current_exe().unwrap()).create_device("cuda");

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Sefirot - Particles Example")
        .with_inner_size(winit::dpi::PhysicalSize::new(SIZE, SIZE))
        .with_resizable(false)
        .build(&event_loop)
        .unwrap();
    let swapchain = device.create_swapchain(
        &window,
        &device.default_stream(),
        SIZE,
        SIZE,
        false,
        false,
        3,
    );
    let display = device.create_tex2d::<Vec4<f32>>(swapchain.pixel_storage(), SIZE, SIZE, 1);

    let particles = Emanation::<Particles>::new();
    let mut particle_data = Vec::<Particle>::new();
    for i in 0..100 {
        for j in 0..100 {
            particle_data.push(Particle {
                position: Vec2::new(i as f32, j as f32),
                velocity: Vec2::new(rand::random(), rand::random()),
            });
        }
    }
    let index = particles.create_index(particle_data.len() as u32);
    let ParticleMapped { position, velocity } =
        particles.create_aos_fields(&device, index, Some("particle-"), &particle_data);

    let update_kernel = particles.build_kernel::<fn(f32)>(
        &device,
        Box::new(index),
        track!(&|el, dt| {
            position[el] += velocity[el] * dt;
            if (position[el] >= 0.0).all() && (position[el] < SIZE as f32).all() {
                display.write(position[el].cast_u32(), Vec4::splat(1.0));
            }
        }),
    );
    let clear_kernel = device.create_kernel_async::<fn()>(&|| {
        display.write(dispatch_id().xy(), Vec3::splat_expr(0.0_f32).extend(1.0));
    });
    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                *control_flow = ControlFlow::Exit;
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                device
                    .default_stream()
                    .scope()
                    .present(&swapchain, &display);
                let mut graph = ComputeGraph::new();
                let clear = graph.add(clear_kernel.dispatch_async([SIZE, SIZE, 1])).id();
                graph.add(update_kernel.dispatch(&1.0)).after(clear);
                graph.execute(&device);
                window.request_redraw();
            }
            _ => {}
        }
    });
}
