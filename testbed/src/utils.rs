use std::sync::LazyLock;

use keter::lang::types::vector::{Vec2, Vec3, Vec4};
use keter::prelude::*;

pub fn bayer(n: usize) -> Vec<u16> {
    assert!(n <= 256);
    if n == 0 {
        panic!("Bayer matrix of order 0 is not defined");
    } else if n == 1 {
        return vec![0];
    }
    let n2 = n / 2;
    let prev_matrix = bayer(n2);
    let mut next_matrix = vec![0; n * n];
    for i in 0..n2 {
        for j in 0..n2 {
            let v = prev_matrix[i * n2 + j] * 4;
            next_matrix[i * n + j] = v;
            next_matrix[i * n + (j + n2)] = v + 2;
            next_matrix[(i + n2) * n + j] = v + 3;
            next_matrix[(i + n2) * n + (j + n2)] = v + 1;
        }
    }
    next_matrix
}

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

static DRAW_LINE_KERNEL: LazyLock<Kernel<fn(Tex2d<Vec4<f32>>, Vec2<f32>, Vec2<f32>, Vec4<f32>)>> =
    LazyLock::new(|| {
        DEVICE.create_kernel::<fn(Tex2d<Vec4<f32>>, Vec2<f32>, Vec2<f32>, Vec4<f32>)>(&track!(
            |display, start, end, color| {
                let t = dispatch_id().x.cast_f32() / (dispatch_size().x - 1).cast_f32();
                let pos = start + (end - start) * t;
                display.write(pos.cast_i32().cast_u32(), color);
            }
        ))
    });
pub fn draw_line(display: &Tex2d<Vec4<f32>>, start: Vec2<f32>, end: Vec2<f32>, color: Vec4<f32>) {
    DRAW_LINE_KERNEL.dispatch(
        [display.width().max(display.height()), 1, 1],
        display,
        &start,
        &end,
        &color,
    );
}
