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

fn main() {}
