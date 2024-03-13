use luisa::lang::types::vector::{Vec2, Vec3};
use luisa::lang::types::AtomicRef;

use super::cache::CachedMapping;
use crate::internal_prelude::*;

pub trait IntoBuffer<V: Value> {
    fn into_buffer(self, device: &Device, count: u32) -> (BufferView<V>, Option<Buffer<V>>);
}
impl<V: Value> IntoBuffer<V> for &[V] {
    fn into_buffer(self, device: &Device, count: u32) -> (BufferView<V>, Option<Buffer<V>>) {
        debug_assert_eq!(self.len() as u32, count);
        let buffer = device.create_buffer_from_slice(self);
        (buffer.clone(), Some(buffer))
    }
}
impl<V: Value> IntoBuffer<V> for &Vec<V> {
    fn into_buffer(self, device: &Device, count: u32) -> (BufferView<V>, Option<Buffer<V>>) {
        self.as_slice().into_buffer(device, count)
    }
}
impl<V: Value, F> IntoBuffer<V> for F
where
    F: FnMut(u32) -> V,
{
    fn into_buffer(mut self, device: &Device, count: u32) -> (BufferView<V>, Option<Buffer<V>>) {
        let buffer = device.create_buffer_from_fn(count as usize, |x| self(x as u32));
        (buffer.clone(), Some(buffer))
    }
}
impl<V: Value> IntoBuffer<V> for () {
    fn into_buffer(self, device: &Device, count: u32) -> (BufferView<V>, Option<Buffer<V>>) {
        let buffer = device.create_buffer(count as usize);
        (buffer.clone(), Some(buffer))
    }
}
impl<V: Value> IntoBuffer<V> for Buffer<V> {
    fn into_buffer(self, _device: &Device, count: u32) -> (BufferView<V>, Option<Buffer<V>>) {
        debug_assert_eq!(self.len(), count as usize);
        (self.clone(), Some(self))
    }
}
impl<V: Value> IntoBuffer<V> for BufferView<V> {
    fn into_buffer(self, _device: &Device, count: u32) -> (BufferView<V>, Option<Buffer<V>>) {
        debug_assert_eq!(self.len(), count as usize);
        (self, None)
    }
}

pub struct BufferMapping<V: Value> {
    pub buffer: BufferView<V>,
    pub handle: Option<Buffer<V>>,
}
impl<V: Value> CachedMapping<V, Expr<u32>> for BufferMapping<V> {
    fn get_expr(&self, index: &Expr<u32>, _ctx: &mut Context, _binding: FieldHandle) -> Expr<V> {
        self.buffer.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<u32>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) {
        self.buffer.write(*index, value);
    }
}
impl<V: Value> Mapping<AtomicRef<V>, Expr<u32>> for BufferMapping<V> {
    fn access(&self, index: &Expr<u32>, _ctx: &mut Context, _binding: FieldHandle) -> AtomicRef<V> {
        self.buffer.atomic_ref(*index)
    }
}

pub struct Tex2dMapping<V: IoTexel> {
    pub texture: Tex2d<V>,
    pub handle: Option<Tex2d<V>>,
}
impl<V: IoTexel> CachedMapping<V, Expr<Vec2<u32>>> for Tex2dMapping<V> {
    fn get_expr(
        &self,
        index: &Expr<Vec2<u32>>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) -> Expr<V> {
        self.texture.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<Vec2<u32>>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) {
        self.texture.write(*index, value);
    }
}

pub struct Tex3dMapping<V: IoTexel> {
    pub texture: Tex3d<V>,
    pub handle: Option<Tex3d<V>>,
}
impl<V: IoTexel> CachedMapping<V, Expr<Vec3<u32>>> for Tex3dMapping<V> {
    fn get_expr(
        &self,
        index: &Expr<Vec3<u32>>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) -> Expr<V> {
        self.texture.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<Vec3<u32>>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) {
        self.texture.write(*index, value);
    }
}
