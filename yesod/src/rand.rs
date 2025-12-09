use keter::lang::types::vector::Vec3;
use keter::prelude::*;

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
