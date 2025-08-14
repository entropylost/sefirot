use keter::lang::types::vector::{Vec2, Vec3};
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
