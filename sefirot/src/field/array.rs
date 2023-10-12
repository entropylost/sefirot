use luisa::lang::types::vector::Vec2;
use luisa::prelude::track;

use crate::domain::{IndexDomain, IndexEmanation};

use super::*;

pub mod structure;

impl<T: EmanationType> Emanation<T> {
    pub fn create_index(&self, length: u32) -> ArrayIndex<T> {
        ArrayIndex {
            field: self.create_field(Some("index")),
            size: length,
        }
    }
    pub fn create_index2d(&self, size: [u32; 2]) -> Array2dIndex<T> {
        Array2dIndex {
            field: self.create_field(Some("index")),
            size,
        }
    }
    pub fn create_array_field<V: Value>(
        &self,
        device: &Device,
        index: ArrayIndex<T>,
        name: Option<&str>,
        values: &[V],
    ) -> Field<Expr<V>, T> {
        assert_eq!(values.len(), index.size as usize);
        let buffer = device.create_buffer_from_slice(values);
        self.create_array_field_from_buffer(index, name, buffer)
    }
    pub fn create_array_field_from_buffer<V: Value>(
        &self,
        index: ArrayIndex<T>,
        name: Option<&str>,
        buffer: Buffer<V>,
    ) -> Field<Expr<V>, T> {
        assert_eq!(buffer.len(), index.size as usize);
        self.create_bound_field(name, BufferAccessor { index, buffer })
    }

    pub fn create_tex2d_field<V: IoTexel>(
        &self,
        device: &Device,
        storage: PixelStorage,
        index: Array2dIndex<T>,
        name: Option<&str>,
    ) -> Field<Expr<V>, T> {
        let texture = device.create_tex2d(storage, index.size[0], index.size[1], 1);
        self.create_bound_field(name, Tex2dAccessor { index, texture })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayIndex<T: EmanationType> {
    pub field: Field<Expr<u32>, T>,
    pub size: u32,
}
impl<T: EmanationType> Deref for ArrayIndex<T> {
    type Target = Field<Expr<u32>, T>;
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<T: EmanationType> IndexEmanation<Expr<u32>> for ArrayIndex<T> {
    type T = T;
    fn bind_fields(&self, idx: Expr<u32>, element: &Element<T>) {
        element.bind(self.field, ExprAccessor::new(idx));
    }
}
impl<T: EmanationType> IndexDomain for ArrayIndex<T> {
    type I = Expr<u32>;
    fn get_index(&self) -> Self::I {
        dispatch_id().x
    }
    fn dispatch_size(&self) -> [u32; 3] {
        [self.size, 1, 1]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Array2dIndex<T: EmanationType> {
    pub field: Field<Expr<Vec2<u32>>, T>,
    pub size: [u32; 2],
}
impl<T: EmanationType> Deref for Array2dIndex<T> {
    type Target = Field<Expr<Vec2<u32>>, T>;
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<T: EmanationType> IndexEmanation<Expr<Vec2<u32>>> for Array2dIndex<T> {
    type T = T;
    fn bind_fields(&self, idx: Expr<Vec2<u32>>, element: &Element<T>) {
        element.bind(self.field, ExprAccessor::new(idx));
    }
}
impl<T: EmanationType> IndexDomain for Array2dIndex<T> {
    type I = Expr<Vec2<u32>>;
    fn get_index(&self) -> Self::I {
        dispatch_id().xy()
    }
    fn dispatch_size(&self) -> [u32; 3] {
        [self.size[0], self.size[1], 1]
    }
}
impl<T: EmanationType> Array2dIndex<T> {
    pub fn morton(&self, emanation: &mut Emanation<T>) -> ArrayIndex<T> {
        assert_eq!(
            self.size[0], self.size[1],
            "Morton indexing only supports square arrays."
        );
        assert!(
            self.size[0].is_power_of_two(),
            "Morton indexing only supports power-of-two arrays."
        );
        assert!(
            self.size[0] < 1 << 16,
            "Morton indexing only supports arrays with size < 65536."
        );
        let name = emanation
            .name_of(self.field)
            .map(|x| format!("{}-morton", x))
            .unwrap_or("index2d-morton".to_string());
        let field = self.field;
        let field = emanation.create_bound_field(
            Some(&name),
            ExprFnAccessor::new(track!(move |el| {
                // https://graphics.stanford.edu/%7Eseander/bithacks.html#InterleaveBMN
                let index = el.get(field).unwrap();
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
            })),
        );
        ArrayIndex {
            field,
            size: self.size[0] * self.size[0],
        }
    }
}

pub struct BufferAccessor<V: Value, T: EmanationType> {
    pub index: ArrayIndex<T>,
    pub buffer: Buffer<V>,
}
impl<V: Value, T: EmanationType> Accessor<T> for BufferAccessor<V, T> {
    type V = Expr<V>;
    type C = Var<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        if let Some(cache) = self.get_cache(element, field) {
            Ok(cache.load())
        } else {
            let value = self.buffer.read(element.get(self.index.field)?);
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
            cache.store(value.clone());
        } else {
            self.insert_cache(element, field, value.var());
        }
        Ok(())
    }

    fn save(&self, element: &Element<T>, field: Field<Self::V, T>) {
        self.buffer.write(
            element.get(self.index.field).unwrap(),
            self.get_cache(element, field).unwrap().load(),
        );
    }

    fn can_write(&self) -> bool {
        true
    }
}

pub struct Tex2dAccessor<V: IoTexel, T: EmanationType> {
    pub index: Array2dIndex<T>,
    pub texture: Tex2d<V>,
}
impl<V: IoTexel, T: EmanationType> Accessor<T> for Tex2dAccessor<V, T> {
    type V = Expr<V>;
    type C = Var<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        if let Some(cache) = self.get_cache(element, field) {
            Ok(cache.load())
        } else {
            let value = self.texture.read(element.get(self.index.field)?);
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
            cache.store(value.clone());
        } else {
            self.insert_cache(element, field, value.var());
        }
        Ok(())
    }

    fn save(&self, element: &Element<T>, field: Field<Self::V, T>) {
        self.texture.write(
            element.get(self.index.field).unwrap(),
            self.get_cache(element, field).unwrap().load(),
        );
    }

    fn can_write(&self) -> bool {
        true
    }
}
