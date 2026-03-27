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
        // TODO: Max unnecessary?
        let vsin = keter::max(0.0, 1.0 - vcos * vcos).sqrt();

        (h.direction() * vsin).extend(vcos)
    }
}

/// Project a random point in [0, 1]^2 to a line on the unit sphere.
#[tracked]
pub fn project_line(uv: Expr<Vec2<f32>>) -> Expr<Vec3<f32>> {
    SphericalEncoder::decode(Vec2::expr(uv.x * 0.5, uv.y))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClarbergEncoder;
impl DirectionEncoder for ClarbergEncoder {
    // https://github.com/mmp/pbrt-v4/blob/8c19f304558fd7681e2fef2c395a689d0106fb05/src/pbrt/util/math.cpp#L292
    #[tracked]
    fn encode(dir: Expr<Vec3<f32>>) -> Expr<Vec2<f32>> {
        let adir = dir.abs();
        let r = (1.0 - adir.z).sqrt();
        let a = keter::max(adir.x, adir.y);
        let b = keter::min(adir.x, adir.y);
        // let b = if a == 0.0 { 0.0 } else { b / a };
        let phi = (b.atan2(a) * (2.0 / PI)).var();

        if adir.x < adir.y {
            *phi = 1.0 - phi;
        }
        let v = phi * r;
        let uv = Vec2::expr(r - v, v).var();

        if dir.z < 0.0 {
            *uv = 1.0 - uv.yx();
        }
        let uv = uv.copysign(dir.xy());

        uv * 0.5 + 0.5
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
        let horiz = phi.direction().copysign(uv);
        (horiz * r * keter::max(2.0 - r.sqr(), 0.0).sqrt()).extend(z)
    }
}
