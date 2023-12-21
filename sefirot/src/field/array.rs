use luisa::lang::types::vector::Vec2;
use luisa::lang::types::AtomicRef;
use parking_lot::Mutex;

use crate::domain::{IndexDomain, IndexEmanation};

use super::*;

pub mod structure;

mod index;
pub use index::*;

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

pub trait IntoTex2d<V: IoTexel> {
    fn into_tex2d(
        self,
        device: &Device,
        width: u32,
        height: u32,
    ) -> (Tex2dView<V>, Option<Tex2d<V>>);
}
impl<V: IoTexel> IntoTex2d<V> for PixelStorage {
    fn into_tex2d(
        self,
        device: &Device,
        width: u32,
        height: u32,
    ) -> (Tex2dView<V>, Option<Tex2d<V>>) {
        let texture = device.create_tex2d(self, width, height, 1);
        (texture.view(0), Some(texture))
    }
}
impl<V: IoTexel> IntoTex2d<V> for Tex2d<V> {
    fn into_tex2d(
        self,
        _device: &Device,
        width: u32,
        height: u32,
    ) -> (Tex2dView<V>, Option<Tex2d<V>>) {
        debug_assert_eq!(self.width(), width);
        debug_assert_eq!(self.height(), height);
        (self.view(0), Some(self))
    }
}
impl<V: IoTexel> IntoTex2d<V> for Tex2dView<V> {
    fn into_tex2d(
        self,
        _device: &Device,
        _width: u32,
        _height: u32,
    ) -> (Tex2dView<V>, Option<Tex2d<V>>) {
        (self, None)
    }
}

impl<V: Value, T: EmanationType> Reference<'_, EField<V, T>> {
    pub fn bind_array(self, index: impl LinearIndex<T>, values: impl IntoBuffer<V>) -> Self {
        let (buffer, handle) = values.into_buffer(self.device(), index.size());
        let accessor = BufferAccessor {
            index: index.reduce(),
            buffer,
            handle,
            atomic: Mutex::new(None),
        };
        self.bind(accessor)
    }
    pub fn buffer(self) -> Option<BufferView<V>> {
        self.accessor().and_then(|a| {
            a.clone()
                .as_any()
                .downcast_ref::<BufferAccessor<V, T>>()
                .map(|a| a.buffer.clone())
        })
    }
    pub fn bind_tex2d(self, index: impl PlanarIndex<T>, values: impl IntoTex2d<V>) -> Self
    where
        V: IoTexel,
    {
        let (texture, handle) = values.into_tex2d(self.device(), index.size().x, index.size().y);
        let accessor = Tex2dAccessor {
            index: index.reduce(),
            texture,
            handle,
        };
        self.bind(accessor)
    }
}

pub struct BufferAccessor<V: Value, T: EmanationType> {
    pub index: ReducedIndex<T>,
    pub buffer: BufferView<V>,
    /// Used to prevent the buffer from being dropped.
    pub handle: Option<Buffer<V>>,
    atomic: Mutex<Option<RawFieldHandle>>,
}
impl<V: Value, T: EmanationType> Accessor<T> for BufferAccessor<V, T> {
    type V = Expr<V>;
    type C = Var<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        if let Some(cache) = self.get_cache(element, field) {
            Ok(cache.load())
        } else {
            let value = self.buffer.var().read(element.get(*self.index)?);
            self.insert_cache(element, field, value.var());
            Ok(value)
        }
    }
    fn set(
        &self,
        element: &Element<T>,
        field: Field<Self::V, T>,
        value: &Self::V,
    ) -> Result<(), WriteError> {
        if let Some(cache) = self.get_cache(element, field) {
            cache.store(value);
        } else {
            self.insert_cache(element, field, value.var());
        }
        Ok(())
    }

    fn save(&self, element: &Element<T>, field: Field<Self::V, T>) {
        self.buffer.var().write(
            element.get(*self.index).unwrap(),
            self.get_cache(element, field).unwrap().load(),
        );
    }

    fn can_write(&self) -> bool {
        true
    }

    fn get_atomic(&self, emanation: &Emanation<T>) -> Option<RawFieldHandle> {
        if let Some(&handle) = self.atomic.lock().as_ref() {
            return Some(handle);
        }
        let handle = emanation
            .create_field("")
            .bind(AtomicBufferAccessor {
                index: self.index,
                buffer: self.buffer.clone(),
            })
            .raw();
        *self.atomic.lock() = Some(handle);
        Some(handle)
    }
}

pub struct AtomicBufferAccessor<V: Value, T: EmanationType> {
    pub index: ReducedIndex<T>,
    pub buffer: BufferView<V>,
}
impl<V: Value, T: EmanationType> Accessor<T> for AtomicBufferAccessor<V, T> {
    type V = AtomicRef<V>;
    type C = AtomicRef<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        Ok(*self.get_or_insert_cache(element, field, || {
            let index = element.get(*self.index).unwrap();
            self.buffer.var().atomic_ref(index)
        }))
    }
    fn set(
        &self,
        _element: &Element<T>,
        _field: Field<Self::V, T>,
        _value: &Self::V,
    ) -> Result<(), WriteError> {
        Err(WriteError {
            message: "Cannot write to `AtomicRef` field. Use atomic operations instead."
                .to_string(),
        })
    }

    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {
        unreachable!();
    }

    fn can_write(&self) -> bool {
        false
    }
}

pub struct Tex2dAccessor<V: IoTexel, T: EmanationType> {
    pub index: ReducedIndex2d<T>,
    pub texture: Tex2dView<V>,
    /// Used to prevent the buffer from being dropped.
    pub handle: Option<Tex2d<V>>,
}
impl<V: IoTexel, T: EmanationType> Accessor<T> for Tex2dAccessor<V, T> {
    type V = Expr<V>;
    type C = Var<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        if let Some(cache) = self.get_cache(element, field) {
            Ok(cache.load())
        } else {
            let value = self.texture.var().read(element.get(*self.index)?);
            self.insert_cache(element, field, value.var());
            Ok(value)
        }
    }
    fn set(
        &self,
        element: &Element<T>,
        field: Field<Self::V, T>,
        value: &Self::V,
    ) -> Result<(), WriteError> {
        if let Some(cache) = self.get_cache(element, field) {
            cache.store(value);
        } else {
            self.insert_cache(element, field, value.var());
        }
        Ok(())
    }

    fn save(&self, element: &Element<T>, field: Field<Self::V, T>) {
        self.texture.var().write(
            element.get(*self.index).unwrap(),
            self.get_cache(element, field).unwrap().load(),
        );
    }

    fn can_write(&self) -> bool {
        true
    }
}
