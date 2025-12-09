use keter::lang::types::vector::{Vec2, Vec3};
use keter::lerp;
use keter::prelude::*;

pub const AXIS_COLORS: [Vec3<f32>; 4] = [
    Vec3::new(0.64178, 0.22938, 0.33132), // 0
    Vec3::new(0.47086, 0.33081, 0.08135), // 90
    Vec3::new(0.06965, 0.44936, 0.3549),  // 180
    Vec3::new(0.23872, 0.32924, 0.72414), // 270
];

#[tracked]
pub fn debug_color_2d(v: Expr<Vec2<f32>>) -> Expr<Vec3<f32>> {
    keter::max(v.x, 0.0) * AXIS_COLORS[0]
        + keter::max(v.y, 0.0) * AXIS_COLORS[1]
        + keter::max(-v.x, 0.0) * AXIS_COLORS[2]
        + keter::max(-v.y, 0.0) * AXIS_COLORS[3]
}

#[tracked]
pub fn debug_color_1d(v: Expr<f32>) -> Expr<Vec3<f32>> {
    keter::max(v, 0.0) * AXIS_COLORS[0] + keter::max(-v, 0.0) * AXIS_COLORS[2]
}

#[tracked]
pub fn reinhard(x: Expr<f32>) -> Expr<f32> {
    x / (1.0 + x)
}

#[tracked]
pub fn sample_gradient<const N: usize>(map: colorous::Gradient, t: Expr<f32>) -> Expr<Vec3<f32>> {
    let map = std::array::from_fn::<_, N, _>(|i| {
        let c = map.eval_rational(i, N - 1);
        Vec3::new(c.r, c.g, c.b).map(|c| ((c as f32) / 255.0).powf(2.2))
    });
    let map = map.expr();

    let t = t.clamp(0.0, 0.9999) * N as f32;
    let idx = t.floor();
    let fract = t - idx;
    let idx = idx.cast_u32();
    lerp(map.read(idx), map.read(idx + 1u32), fract)
}

#[tracked]
pub fn inferno(t: Expr<f32>) -> Expr<Vec3<f32>> {
    sample_gradient::<256>(colorous::INFERNO, t)
}
#[tracked]
pub fn plasma(t: Expr<f32>) -> Expr<Vec3<f32>> {
    sample_gradient::<256>(colorous::PLASMA, t)
}

pub const LUMA_WEIGHTS: Vec3<f32> = Vec3::new(0.299, 0.587, 0.114);

#[tracked]
pub fn luma(c: Expr<Vec3<f32>>) -> Expr<f32> {
    c.dot(LUMA_WEIGHTS)
}
#[tracked]
pub fn with_luma(c: Expr<Vec3<f32>>, l: Expr<f32>) -> Expr<Vec3<f32>> {
    let l_orig = luma(c);
    c * (l / l_orig)
}
