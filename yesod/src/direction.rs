use std::f32::consts::{PI, TAU};

use keter::lang::types::vector::{Vec2, Vec3};
use keter::prelude::*;

pub trait DirectionEncoder: 'static + Copy {
    /// Encoded vector between 0 and 1.
    fn encode(dir: Expr<Vec3<f32>>) -> Expr<Vec2<f32>>;
    fn decode(uv: Expr<Vec2<f32>>) -> Expr<Vec3<f32>>;
}

// Check https://knarkowicz.wordpress.com/2014/04/16/octahedron-normal-vector-encoding/ as well.
// https://jcgt.org/published/0003/02/01/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OctahedralEncoder;
impl DirectionEncoder for OctahedralEncoder {
    #[tracked]
    fn encode(dir: Expr<Vec3<f32>>) -> Expr<Vec2<f32>> {
        let n = dir / dir.abs().reduce_sum();
        let encoded = if n.z >= 0.0 {
            n.xy()
        } else {
            (1.0 - n.yx().abs()) * n.xy().signum()
        };
        encoded * 0.5 + 0.5
    }
    #[tracked]
    fn decode(uv: Expr<Vec2<f32>>) -> Expr<Vec3<f32>> {
        let value = uv * 2.0 - 1.0;
        let f = 1.0 - value.abs().reduce_sum();
        let n = if f >= 0.0 {
            value
        } else {
            (1.0 - value.yx().abs()) * value.signum()
        };
        n.extend(f).normalize()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SphericalEncoder;
impl DirectionEncoder for SphericalEncoder {
    #[tracked]
    fn encode(dir: Expr<Vec3<f32>>) -> Expr<Vec2<f32>> {
        Vec2::expr((dir.y.atan2(dir.x) / TAU).fract(), dir.z * 0.5 + 0.5)
    }
    #[tracked]
    fn decode(uv: Expr<Vec2<f32>>) -> Expr<Vec3<f32>> {
        let h = uv.x * TAU;

        let vcos = uv.y * 2.0 - 1.0;
        let vsin = keter::max(0.0, 1.0 - vcos * vcos).sqrt();

        (h.direction() * vsin).extend(vcos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClarbergEncoder;
impl DirectionEncoder for ClarbergEncoder {
    #[tracked]
    fn encode(dir: Expr<Vec3<f32>>) -> Expr<Vec2<f32>> {
        let phi = dir.y.atan2(dir.x);

        Vec2::splat_expr(0.0)
    }
    // https://www.pbr-book.org/4ed/Geometry_and_Transformations/Spherical_Geometry#EqualAreaSquareToSphere
    #[tracked]
    fn decode(uv: Expr<Vec2<f32>>) -> Expr<Vec3<f32>> {
        let uv = uv * 2.0 - 1.0;
        let uv_pos = uv.abs();
        let signed_dist = 1.0 - uv_pos.reduce_sum();
        let dist = signed_dist.abs();
        let r = 1.0 - dist;

        let phi = (if r == 0.0 {
            1.0_f32.expr()
        } else {
            (uv_pos.y - uv_pos.x) / r + 1.0
        }) * (PI / 4.0);

        let z = (1.0 - r.sqr()).copysign(signed_dist);
        (phi.direction().copysign(uv) * r * keter::max(2.0 - r.sqr(), 0.0).sqrt()).extend(z)
    }
}
