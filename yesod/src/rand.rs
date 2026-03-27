use std::f32::consts::PI;

use keter::lang::types::vector::{Vec2, Vec3};
use keter::prelude::*;

// TODO: https://www.reedbeta.com/blog/hash-functions-for-gpu-rendering/

// https://github.com/markjarzynski/PCG3D/blob/master/pcg3d.hlsl
#[tracked]
pub fn pcg3d(v: Expr<Vec3<u32>>) -> Expr<Vec3<u32>> {
    let v = v.var();
    *v = v * 1664525u32 + 1013904223u32;

    *v.x += v.y * v.z;
    *v.y += v.z * v.x;
    *v.z += v.x * v.y;

    *v ^= v >> 16u32;

    *v.x += v.y * v.z;
    *v.y += v.z * v.x;
    *v.z += v.x * v.y;

    **v
}

#[tracked]
pub fn pcg(v: Expr<u32>) -> Expr<u32> {
    let state = v * 747796405u32 + 2891336453u32;
    let word = ((state >> ((state >> 28u32) + 4u32)) ^ state) * 277803737u32;
    (word >> 22u32) ^ word
}

#[tracked]
pub fn pcgf(v: Expr<u32>) -> Expr<f32> {
    pcg(v).cast_f32() / u32::MAX as f32
}

#[tracked]
pub fn pcg3df(v: Expr<Vec3<u32>>) -> Expr<Vec3<f32>> {
    pcg3d(v).cast_f32() / u32::MAX as f32
}

// Solutions of x^(n + 2) = x + 1
#[allow(clippy::excessive_precision)]
pub const GOLDEN_ROOTS: [f64; 3] = [
    1.6180339887498948482,
    1.32471795724474602596,
    1.22074408460575947536,
];

// https://extremelearning.com.au/unreasonable-effectiveness-of-quasirandom-sequences
// https://www.martysmods.com/a-better-r2-sequence/
/*
#[tracked]
pub fn r2(t: Expr<u32>) -> Expr<Vec2<f32>> {
    let ig = 1.0 / GOLDEN_ROOTS[1];
    let a = Vec2::new((1.0 - ig) as f32, (1.0 - ig * ig) as f32);
    (t.cast_f32() * a).fract()
}
*/

#[tracked]
pub fn r2(t: Expr<u32>) -> Expr<Vec2<f32>> {
    let ig = 1.0 / GOLDEN_ROOTS[1];
    let max = u32::MAX as f64 + 1.0;
    let a = Vec2::new((ig * max) as u32, (ig * ig * max) as u32);
    (t * a).cast_f32() / max as f32
}

// https://extremelearning.com.au/a-simple-method-to-construct-isotropic-quasirandom-blue-noise-point-sequences/
#[tracked]
pub fn r2blue(t: Expr<u32>) -> Expr<Vec2<f32>> {
    let r2_v = r2(t);
    let l = 0.38 / (2.0 * (t.cast_f32() + 1.0 - 0.7).sqrt());
    // TODO: This isn't centered.
    let offset = PI.sqrt() * pcg3df(Vec3::expr(t, 3928, 1731)).xy();
    (r2_v + l * offset).fract()
}
