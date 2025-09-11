use std::ops::Deref;
use std::sync::Arc;

use luisa_compute::lang::types::vector::Vec2;
use luisa_compute::lang::types::AtomicRef;
use num_traits::Float;
use parking_lot::Mutex;

use crate::graph::NodeConfigs;
use crate::prelude::*;

/// A struct that runs a given function upon drop.
#[derive(Debug, Clone)]
pub struct FnRelease<F: FnOnce() + 'static>(Option<F>);
impl<F: FnOnce() + 'static> FnRelease<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}
impl<F: FnOnce() + 'static> Drop for FnRelease<F> {
    fn drop(&mut self) {
        self.0.take().unwrap()();
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Singleton<V: Value>(pub Buffer<V>);
impl<V: Value> Deref for Singleton<V> {
    type Target = SingletonVar<V>;
    fn deref(&self) -> &Self::Target {
        #[allow(clippy::needless_lifetimes)]
        unsafe fn cast_ptr<'a, T, S>(x: &'a T) -> &'a S {
            &*(x as *const T as *const S)
        }
        let buffer_var = &**self.0;
        unsafe { cast_ptr::<BufferVar<V>, SingletonVar<V>>(buffer_var) }
    }
}
impl<V: Value> Singleton<V> {
    #[allow(clippy::new_without_default)]
    pub fn new(value: V) -> Self {
        Self(DEVICE.create_buffer_from_slice(&[value]))
    }
    pub fn write_host(&self, value: V) -> NodeConfigs<'static>
    where
        V: Send,
    {
        self.0.copy_from_vec(vec![value])
    }
    pub fn read_to(&self, dst: &Arc<Mutex<V>>) -> NodeConfigs<'static>
    where
        V: Send,
    {
        let dst = dst.clone();
        let src = self.0.clone();
        let mut guard = dst.lock_arc();
        let dst = unsafe { std::mem::transmute::<&mut V, &'static mut V>(&mut *guard) };
        let dst_slice = std::slice::from_mut(dst);
        src.copy_to_async(dst_slice).release(guard)
    }
    pub fn read_blocking(&self) -> V {
        self.0.copy_to_vec()[0]
    }
}

#[repr(transparent)]
pub struct SingletonVar<V: Value>(pub BufferVar<V>);
impl<V: Value> SingletonVar<V> {
    pub fn read(&self) -> Expr<V> {
        self.0.read(0)
    }
    pub fn write(&self, value: impl AsExpr<Value = V>) {
        self.0.write(0, value.as_expr())
    }
    pub fn atomic(&self) -> AtomicRef<V> {
        self.0.atomic_ref(0)
    }
}

pub trait Angle {
    type Output;
    fn angle(self) -> Self::Output;
}

impl Angle for Vec2<f32> {
    type Output = f32;
    fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
}
impl Angle for Vec2<f16> {
    type Output = f16;
    fn angle(self) -> f16 {
        Float::atan2(self.y, self.x)
    }
}

impl Angle for Expr<Vec2<f32>> {
    type Output = Expr<f32>;
    fn angle(self) -> Expr<f32> {
        self.y.atan2(self.x)
    }
}
impl Angle for Expr<Vec2<f16>> {
    type Output = Expr<f16>;
    fn angle(self) -> Expr<f16> {
        self.y.atan2(self.x)
    }
}

pub trait Direction {
    type Output;
    fn direction(self) -> Self::Output;
}

impl Direction for f32 {
    type Output = Vec2<f32>;
    fn direction(self) -> Vec2<f32> {
        Vec2::new(self.cos(), self.sin())
    }
}
impl Direction for f16 {
    type Output = Vec2<f16>;
    fn direction(self) -> Vec2<f16> {
        Vec2::new(Float::cos(self), Float::sin(self))
    }
}
impl Direction for Expr<f32> {
    type Output = Expr<Vec2<f32>>;
    fn direction(self) -> Expr<Vec2<f32>> {
        Vec2::expr(self.cos(), self.sin())
    }
}
impl Direction for Expr<f16> {
    type Output = Expr<Vec2<f16>>;
    fn direction(self) -> Expr<Vec2<f16>> {
        Vec2::expr(self.cos(), self.sin())
    }
}

#[derive(Debug, BindGroup)]
pub struct Buffer2d<V: Value> {
    buffer: Buffer<V>,
    size: Vec2<u32>,
}
impl<V: Value> Buffer2d<V> {
    pub fn new(device: &Device, size: impl Into<Vec2<u32>>) -> Self {
        let size = size.into();
        let buffer = device.create_buffer::<V>(size.x as usize * size.y as usize);
        Self { buffer, size }
    }
    pub fn width(&self) -> u32 {
        self.size.x
    }
    pub fn height(&self) -> u32 {
        self.size.y
    }
    pub fn size(&self) -> Vec2<u32> {
        self.size
    }
    pub fn buffer(&self) -> &Buffer<V> {
        &self.buffer
    }
    pub fn var(&self) -> Buffer2dVar<V> {
        Buffer2dVar {
            buffer: self.buffer.var(),
            size: self.size.expr(),
        }
    }
}
impl<V: Value> Buffer2dVar<V> {
    pub fn buffer(&self) -> &BufferVar<V> {
        &self.buffer
    }
    pub fn size(&self) -> Expr<Vec2<u32>> {
        self.size
    }
    #[luisa_compute::prelude::tracked]
    pub fn index(&self, coord: impl AsExpr<Value = Vec2<u32>>) -> Expr<u32> {
        let coord = coord.as_expr();
        coord.x * self.size.y.as_expr() + coord.y
    }
    #[luisa_compute::prelude::tracked]
    pub fn index_wrapping(&self, coord: impl AsExpr<Value = Vec2<i32>>) -> Expr<u32> {
        let coord = coord.as_expr();
        let size = self.size.as_expr().cast::<i32>();
        let wrapped_coord = coord.rem_euclid(size).cast_u32();
        wrapped_coord.x * self.size.y.as_expr() + wrapped_coord.y
    }
    pub fn read(&self, coord: impl AsExpr<Value = Vec2<u32>>) -> Expr<V> {
        self.buffer.read(self.index(coord))
    }
    pub fn read_wrapping(&self, coord: impl AsExpr<Value = Vec2<i32>>) -> Expr<V> {
        self.buffer.read(self.index_wrapping(coord))
    }
    pub fn write(&self, coord: impl AsExpr<Value = Vec2<u32>>, value: impl AsExpr<Value = V>) {
        self.buffer.write(self.index(coord), value.as_expr());
    }
    pub fn write_wrapping(
        &self,
        coord: impl AsExpr<Value = Vec2<i32>>,
        value: impl AsExpr<Value = V>,
    ) {
        self.buffer
            .write(self.index_wrapping(coord), value.as_expr());
    }
    pub fn atomic(&self, coord: impl AsExpr<Value = Vec2<u32>>) -> AtomicRef<V> {
        self.buffer.atomic_ref(self.index(coord))
    }
    pub fn atomic_wrapping(&self, coord: impl AsExpr<Value = Vec2<i32>>) -> AtomicRef<V> {
        self.buffer.atomic_ref(self.index_wrapping(coord))
    }
}
