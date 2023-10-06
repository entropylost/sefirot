use std::env::current_exe;

use luisa::lang::types::vector::Vec2;
use sefirot::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Particles {}

impl EmanationType for Particles {}

#[derive(Debug, Clone, Copy, PartialEq, Value, Structure)]
#[repr(C)]
struct Particle {
    position: Vec2<f32>,
    velocity: Vec2<f32>,
}

fn main() {
    luisa::init_logger();
    let device = Context::new(current_exe().unwrap()).create_device("cuda");

    let mut particles = Emanation::<Particles>::new();
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
        particles.create_aos_fields(&device, &index, None::<String>, &particle_data);

    let kernel = particles.build_kernel(&device, index, |el: &Element<Particles>| {
        track! {
        *position(el) = *position(el) + *velocity(el);
        }
    });
}
