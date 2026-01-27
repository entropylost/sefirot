use std::any::TypeId;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{LazyLock, Mutex};

use keter::lang::types::vector::Vec2;
use keter::prelude::*;
use keter::runtime::{KernelSignature, KernelSignature2};
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

static MAP: LazyLock<Mutex<HashMap<TypeId, &'static Kernel<fn()>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn dispatch0_async<T: Fn()>(x: T, size: [u32; 3]) -> Command<'static, 'static> {
    let map = &*MAP;
    let mut map = map.lock().unwrap();
    let t = typeid::of::<T>();
    let kernel = if let Some(kernel) = map.get(&t) {
        kernel
    } else {
        let kernel = Box::leak(Box::new(DEVICE.create_kernel_async::<fn()>(&x)));
        map.insert(t, kernel);
        &&*kernel
    };
    kernel.dispatch_async(size)
}
