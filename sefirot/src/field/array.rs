use luisa::lang::types::vector::Vec2;
use luisa::lang::types::AtomicRef;

use crate::domain::{IndexDomain, IndexEmanation};

use super::*;

pub mod structure;

impl<T: EmanationType> Emanation<T> {
    pub fn create_index(&self, length: u32) -> ArrayIndex<T> {
        ArrayIndex {
            field: *self.create_field("index"),
            size: length,
        }
    }
    pub fn create_index2d(&self, size: [u32; 2]) -> ArrayIndex2d<T> {
        ArrayIndex2d {
            field: *self.create_field("index2d"),
            size,
        }
    }
}
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

impl<V: Value, T: EmanationType> Reference<'_, EField<V, T>> {
    pub fn bind_array(self, index: ArrayIndex<T>, values: impl IntoBuffer<V>) -> Self {
        let (buffer, handle) = values.into_buffer(self.device(), index.size);
        let accessor = BufferAccessor {
            index,
            buffer,
            handle,
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
    pub fn bind_tex2d(self, index: ArrayIndex2d<T>, storage: PixelStorage) -> Self
    where
        V: IoTexel,
    {
        let texture = self
            .device()
            .create_tex2d(storage, index.size[0], index.size[1], 1);
        let accessor = Tex2dAccessor {
            index,
            texture: texture.view(0),
            handle: Some(texture),
        };
        self.bind(accessor)
    }
}

/// A field marking that a given [`Emanation<T>`] can be mapped to a sized one-dimensional array.
///
/// Also implements [`Domain`] via [`IndexDomain`], which allows [`Kernel`] calls over the array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayIndex<T: EmanationType> {
    pub field: EField<u32, T>,
    pub size: u32,
}
impl<T: EmanationType> From<ArrayIndex<T>> for EField<u32, T> {
    fn from(index: ArrayIndex<T>) -> Self {
        index.field
    }
}

impl<T: EmanationType> IndexEmanation<Expr<u32>> for ArrayIndex<T> {
    type T = T;
    fn bind_fields(&self, idx: Expr<u32>, element: &Element<T>) {
        element.bind(self.field, ValueAccessor(idx));
    }
}
impl<T: EmanationType> IndexDomain for ArrayIndex<T> {
    type I = Expr<u32>;
    type A = ();
    fn get_index(&self) -> Self::I {
        dispatch_id().x
    }
    fn dispatch_size(&self, _: ()) -> [u32; 3] {
        [self.size, 1, 1]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayIndex2d<T: EmanationType> {
    pub field: EField<Vec2<u32>, T>,
    pub size: [u32; 2],
}
impl<T: EmanationType> From<ArrayIndex2d<T>> for EField<Vec2<u32>, T> {
    fn from(index: ArrayIndex2d<T>) -> Self {
        index.field
    }
}

impl<T: EmanationType> IndexEmanation<Expr<Vec2<u32>>> for ArrayIndex2d<T> {
    type T = T;
    fn bind_fields(&self, idx: Expr<Vec2<u32>>, element: &Element<T>) {
        element.bind(self.field, ValueAccessor(idx));
    }
}
impl<T: EmanationType> IndexDomain for ArrayIndex2d<T> {
    type I = Expr<Vec2<u32>>;
    type A = ();
    fn get_index(&self) -> Self::I {
        dispatch_id().xy()
    }
    fn dispatch_size(&self, _: ()) -> [u32; 3] {
        [self.size[0], self.size[1], 1]
    }
}
impl<T: EmanationType> ArrayIndex2d<T> {
    pub fn morton(&self, emanation: &Emanation<T>) -> ArrayIndex<T> {
        assert_eq!(
            self.size[0], self.size[1],
            "Morton indexing only supports square arrays."
        );
        assert!(
            self.size[0].is_power_of_two(),
            "Morton indexing only supports power-of-two arrays."
        );
        assert!(
            self.size[0] <= 1 << 16,
            "Morton indexing only supports arrays with size < 65536."
        );
        let name = emanation.on(self.field).name() + "-morton";

        let field = self.field;
        let field = *emanation.create_field(&name).bind_fn(track!(move |el| {
            // https://graphics.stanford.edu/%7Eseander/bithacks.html#InterleaveBMN
            let index = field[[el]];
            let x = index.x.var();

            *x = (x | (x << 8)) & 0x00ff00ff;
            *x = (x | (x << 4)) & 0x0f0f0f0f; // 0b00001111
            *x = (x | (x << 2)) & 0x33333333; // 0b00110011
            *x = (x | (x << 1)) & 0x55555555; // 0b01010101

            let y = index.y.var();

            *y = (y | (y << 8)) & 0x00ff00ff;
            *y = (y | (y << 4)) & 0x0f0f0f0f; // 0b00001111
            *y = (y | (y << 2)) & 0x33333333; // 0b00110011
            *y = (y | (y << 1)) & 0x55555555; // 0b01010101

            x | (y << 1)
        }));
        ArrayIndex {
            field,
            size: self.size[0] * self.size[0],
        }
    }
}

pub struct BufferAccessor<V: Value, T: EmanationType> {
    pub index: ArrayIndex<T>,
    pub buffer: BufferView<V>,
    /// Used to prevent the buffer from being dropped.
    pub handle: Option<Buffer<V>>,
}
impl<V: Value, T: EmanationType> Accessor<T> for BufferAccessor<V, T> {
    type V = Expr<V>;
    type C = Var<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        if let Some(cache) = self.get_cache(element, field) {
            Ok(cache.load())
        } else {
            let value = self.buffer.var().read(element.get(self.index.field)?);
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
            element.get(self.index.field).unwrap(),
            self.get_cache(element, field).unwrap().load(),
        );
    }

    fn can_write(&self) -> bool {
        true
    }
}

pub struct AtomicBufferAccessor<V: Value, T: EmanationType> {
    pub index: ArrayIndex<T>,
    pub buffer: BufferView<V>,
}
impl<V: Value, T: EmanationType> Accessor<T> for AtomicBufferAccessor<V, T> {
    type V = AtomicRef<V>;
    type C = AtomicRef<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        Ok(*self.get_or_insert_cache(element, field, || {
            let index = element.get(self.index.field).unwrap();
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
impl<'a, V: Value, T: EmanationType> Reference<'a, EField<V, T>> {
    /// Creates a [`Field`] that can be used to perform atomic operations on the values of this [`Field`].
    /// Panics if this [`Field`] is not bound to a [`BufferAccessor`] or a [`StructArrayAccessor`].
    pub fn atomic(self) -> Reference<'a, Field<AtomicRef<V>, T>> {
        let accessor = self.accessor().unwrap();

        let accessor = accessor
            .as_any()
            .downcast_ref::<BufferAccessor<V, T>>()
            .expect("Cannot create atomic reference to non-buffer field.");
        self.emanation
            .create_field(&format!("{}-atomic", self.name()))
            .bind(AtomicBufferAccessor {
                index: accessor.index,
                buffer: accessor.buffer.clone(),
            })
    }
}

pub struct Tex2dAccessor<V: IoTexel, T: EmanationType> {
    pub index: ArrayIndex2d<T>,
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
            let value = self.texture.var().read(element.get(self.index.field)?);
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
            element.get(self.index.field).unwrap(),
            self.get_cache(element, field).unwrap().load(),
        );
    }

    fn can_write(&self) -> bool {
        true
    }
}
