use keter::lang::types::vector::Vec2;
use keter::prelude::*;
use nalgebra::SVector as Vector;

#[inline]
pub fn iter_grid<const D: usize>(shape: Vector<u32, D>) -> impl Iterator<Item = Vector<u32, D>> {
    let total_size = shape.cast::<usize>().product();
    (0..total_size).map(move |i| from_linear(i, shape))
}
#[inline]
pub fn from_linear<const D: usize>(mut index: usize, shape: Vector<u32, D>) -> Vector<u32, D> {
    Vector::from_fn(|i, _| {
        let si = shape[i] as usize;
        let res = index % si;
        index /= si;
        res as u32
    })
}
#[inline]
pub fn to_linear<const D: usize>(index: Vector<u32, D>, shape: Vector<u32, D>) -> usize {
    index
        .zip_fold(&shape, (1, 0), |(step, res), ix, s| {
            (step * s as usize, res + step * ix as usize)
        })
        .1
}

#[tracked]
pub fn next_float_down_pos(x: Expr<f32>) -> Expr<f32> {
    let x: Expr<u32> = x.bitcast();
    let x = x - 1;
    x.bitcast()
}

#[tracked]
pub fn next_float_down_pos_2(a: Expr<Vec2<f32>>) -> Expr<Vec2<f32>> {
    Vec2::expr(next_float_down_pos(a.x), next_float_down_pos(a.y))
}
