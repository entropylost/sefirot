use keter::lang::types::vector::{Vec2, Vec3};
use keter::prelude::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, Value)]
pub struct Bilinear {
    pub base: Vec2<u32>,
    pub fract: Vec2<f32>,
}
impl Bilinear {
    #[tracked]
    pub fn new(pos: Expr<Vec2<f32>>) -> Expr<Self> {
        Self::from_comps_expr(BilinearComps {
            base: pos.cast_u32(),
            fract: pos.fract(),
        })
    }
}
impl BilinearExpr {
    // Probably more optimized if its a generic loop, but not if unrolled.
    #[tracked]
    pub fn get(self, i: Expr<u32>) -> (Expr<Vec2<u32>>, Expr<f32>) {
        let offset = Vec2::expr(i & 1, i >> 1);
        let weight = (offset == 1)
            .select(self.fract, 1.0 - self.fract)
            .reduce_prod();
        // let weight = (offset.cast_f32() - (1.0 - self.fract)).abs().reduce_prod();
        (self.base + offset, weight)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Value)]
pub struct Trilinear {
    pub base: Vec3<u32>,
    // 1 - fract
    pub fract: Vec3<f32>,
}
impl Trilinear {
    #[tracked]
    pub fn new(pos: Expr<Vec3<f32>>) -> Expr<Self> {
        Self::from_comps_expr(TrilinearComps {
            base: pos.cast_u32(),
            fract: pos.fract(),
        })
    }
}
impl TrilinearExpr {
    #[tracked]
    pub fn get(self, i: Expr<u32>) -> (Expr<Vec3<u32>>, Expr<f32>) {
        let offset = Vec3::expr(i & 1, (i >> 1) & 1, i >> 2);
        let weight = (offset == 1)
            .select(self.fract, 1.0 - self.fract)
            .reduce_prod();
        // let weight = (offset.cast_f32() - (1.0 - self.fract)).abs().reduce_prod();
        (self.base + offset, weight)
    }
}
