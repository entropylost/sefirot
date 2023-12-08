use std::env::current_exe;

use luisa::lang::types::vector::{Vec2, Vec3, Vec4};
use sefirot::field::partition::{PartitionFields, PartitionIndex};
use sefirot::graph::{AsNodes, ComputeGraph};
use sefirot::prelude::*;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Value)]
#[repr(u32)]
pub enum Material {
    Default = 0,
    NULL = u32::MAX,
}
impl EmanationType for Material {}

impl PartitionIndex for Material {
    fn to(this: Self) -> u32 {
        this as u32
    }
    fn to_expr(this: Expr<Self>) -> Expr<u32> {
        this.as_u32()
    }
    fn null() -> Self {
        Material::NULL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Value, Structure)]
#[repr(C)]
struct Particle {
    position: Vec2<f32>,
    velocity: Vec2<f32>,
    material: Material,
}

impl EmanationType for Particle {}

const SIZE: u32 = 512;

fn main() {
    luisa::init_logger();
    let device = Context::new(current_exe().unwrap()).create_device("cpu");

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

    let particles = Emanation::<Particle>::new(&device);
    let mut particle_data = Vec::<Particle>::new();
    for i in 0..100 {
        for j in 0..100 {
            let material = if rand::random() {
                Material::Default
            } else {
                Material::NULL
            };
            particle_data.push(Particle {
                position: Vec2::new(i as f32, j as f32),
                velocity: Vec2::new(rand::random(), rand::random()),
                material,
            });
        }
    }
    let index = particles.create_index(particle_data.len() as u32);
    let ParticleMapped {
        position,
        velocity,
        material: material_id,
    } = particles.create_soa_fields(index, "particle-", &particle_data);

    let materials = Emanation::<Material>::new(&device);

    let material_index = materials.create_index(1);
    let material = *particles.map_index(
        &materials,
        *particles.on(material_id).map(|id, _| id.as_u32()),
        material_index,
    );

    let material_parts = particles.partition(
        index,
        &materials,
        material_index,
        PartitionFields {
            const_partition: *particles.create_field(""),
            partition: material_id,
            partition_map: material,
        },
        None,
    );

    let update_kernel = particles.build_kernel_with_domain_args::<fn(f32), _>(
        material_parts.select_dyn(),
        track!(&|el, dt| {
            position[[el]] += velocity[[el]] * dt;
        }),
    );
    let draw_kernel = particles.build_kernel::<fn()>(
        index,
        track!(&|el| {
            if (position[[el]] >= 0.0).all() && (position[[el]] < SIZE as f32).all() {
                display.write(position[[el]].cast_u32(), Vec4::splat(1.0));
            }
        }),
    );
    let clear_kernel = device.create_kernel_async::<fn()>(&|| {
        display.write(dispatch_id().xy(), Vec3::splat_expr(0.0_f32).extend(1.0));
    });

    ComputeGraph::new(&device)
        .add(materials.on(&material_parts).update())
        .execute_clear();

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
                let mut graph = ComputeGraph::new(&device);
                let clear = graph.add_single(clear_kernel.dispatch_async([SIZE, SIZE, 1]));
                graph.add(
                    (
                        update_kernel.dispatch_with_domain_args(Material::Default, &1.0),
                        draw_kernel.dispatch(),
                    )
                        .after(clear),
                );
                graph.execute();
                window.request_redraw();
            }
            _ => {}
        }
    });
}
