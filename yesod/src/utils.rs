use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use keter::lang::types::AtomicRef;
use keter::lang::types::vector::{Vec2, Vec3, Vec4};
use keter::prelude::*;
use keter::runtime::{AsKernelArg, KernelArg};
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

type CacheElement = (
    &'static (dyn Any + Send + Sync),
    &'static (dyn Any + Send + Sync),
);

static CACHE: LazyLock<Mutex<HashMap<TypeId, Vec<CacheElement>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn memo<T: 'static + Send + Sync, S: 'static + Send + Sync + PartialEq, F: FnOnce() -> T>(
    key: S,
    f: F,
) -> &'static T {
    /*println!(
        "Memoizing. Key type: {}, Value type: {}",
        std::any::type_name::<S>(),
        std::any::type_name::<F>()
    );*/
    let cache = &*CACHE;
    let mut cache = cache.lock().unwrap();
    let t = typeid::of::<F>();
    if let Some(values) = cache.get(&t) {
        for (k, v) in values {
            if k.downcast_ref::<S>().unwrap() == &key {
                return v.downcast_ref::<T>().unwrap();
            }
        }
    }
    let value = Box::leak(Box::new(f()));
    cache
        .entry(t)
        .or_insert_with(Vec::new)
        .push((Box::leak(Box::new(key)), value));
    value
}
pub fn once<T: 'static + Send + Sync, F: FnOnce() -> T>(f: F) -> &'static T {
    memo((), f)
}

pub fn dispatch0_async<T: Fn()>(x: T, size: [u32; 3]) -> Command<'static, 'static> {
    once(|| DEVICE.create_kernel_async::<fn()>(&x)).dispatch_async(size)
}
pub fn dispatch1_async<S0: KernelArg + AsKernelArg<Output = S0> + 'static, T: Fn(S0::Parameter)>(
    x: T,
    size: [u32; 3],
    arg0: S0,
) -> Command<'static, 'static> {
    once(|| DEVICE.create_kernel_async::<fn(S0)>(&x)).dispatch_async(size, &arg0)
}
pub fn dispatch0<T: Fn()>(x: T, size: [u32; 3]) {
    DEVICE
        .default_stream()
        .scope()
        .submit([dispatch0_async(x, size)]);
}
pub fn dispatch1<S0: KernelArg + AsKernelArg<Output = S0> + 'static, T: Fn(S0::Parameter)>(
    x: T,
    size: [u32; 3],
    arg0: &S0,
) {
    DEVICE
        .default_stream()
        .scope()
        .submit([once(|| DEVICE.create_kernel_async::<fn(S0)>(&x)).dispatch_async(size, arg0)]);
}

#[tracked]
pub fn encode_morton2_16(a: Expr<Vec2<u32>>) -> Expr<u32> {
    fn interleave(x: Expr<u32>) -> Expr<u32> {
        let x = (x | (x << 8)) & 0x00FF00FF;
        let x = (x | (x << 4)) & 0x0F0F0F0F;
        let x = (x | (x << 2)) & 0x33333333;
        (x | (x << 1)) & 0x55555555
    }
    interleave(a.x) | (interleave(a.y) << 1)
}
#[tracked]
pub fn decode_morton2_16(b: Expr<u32>) -> Expr<Vec2<u32>> {
    let x = b & 0x55555555;
    let y = (b >> 1) & 0x55555555;
    fn deinterleave(x: Expr<u32>) -> Expr<u32> {
        let x = (x | (x >> 1)) & 0x33333333;
        let x = (x | (x >> 2)) & 0x0F0F0F0F;
        let x = (x | (x >> 4)) & 0x00FF00FF;
        (x | (x >> 8)) & 0x0000FFFF
    }
    Vec2::expr(deinterleave(x), deinterleave(y))
}

// https://www.realtimerendering.com/raytracinggems/unofficial_RayTracingGems_v1.9.pdf#page84
#[tracked]
pub fn offset_ray(p: Expr<Vec3<f32>>, n: Expr<Vec3<f32>>) -> Expr<Vec3<f32>> {
    const ORIGIN: f32 = 1.0 / 32.0;
    const FLOAT_SCALE: f32 = 1.0 / 65536.0;
    const INT_SCALE: f32 = 65536.0; // originally 256.0 but that causes artifacts.

    let of_i = (INT_SCALE * n).cast_i32();
    let p_i = p.bitcast::<Vec3<i32>>() + (p < 0.0_f32).select(-of_i, of_i);
    let p_i = p_i.bitcast::<Vec3<f32>>();
    (p.abs() < ORIGIN).select(p + FLOAT_SCALE * n, p_i)
}

pub trait FetchAddVector {
    type Target;
    fn fetch_add(self, target: Self::Target);
}
impl FetchAddVector for AtomicRef<Vec3<f32>> {
    type Target = Expr<Vec3<f32>>;
    fn fetch_add(self, value: Self::Target) {
        self.x.fetch_add(value.x);
        self.y.fetch_add(value.y);
        self.z.fetch_add(value.z);
    }
}
impl FetchAddVector for AtomicRef<Vec4<f32>> {
    type Target = Expr<Vec4<f32>>;
    fn fetch_add(self, value: Self::Target) {
        self.x.fetch_add(value.x);
        self.y.fetch_add(value.y);
        self.z.fetch_add(value.z);
        self.w.fetch_add(value.w);
    }
}
impl FetchAddVector for AtomicRef<Vec3<u32>> {
    type Target = Expr<Vec3<u32>>;
    fn fetch_add(self, value: Self::Target) {
        self.x.fetch_add(value.x);
        self.y.fetch_add(value.y);
        self.z.fetch_add(value.z);
    }
}
impl FetchAddVector for AtomicRef<Vec4<u32>> {
    type Target = Expr<Vec4<u32>>;
    fn fetch_add(self, value: Self::Target) {
        self.x.fetch_add(value.x);
        self.y.fetch_add(value.y);
        self.z.fetch_add(value.z);
        self.w.fetch_add(value.w);
    }
}
