use crate::domain::{IndexDomain, IndexEmanation};

use super::*;

pub mod structure;

impl<T: EmanationType> Emanation<T> {
    pub fn create_index(&mut self, length: u32) -> ArrayIndex<T> {
        ArrayIndex {
            field: self.create_field(Some("index")),
            length,
        }
    }
    pub fn create_array_field<V: Value>(
        &mut self,
        device: &Device,
        index: &ArrayIndex<T>,
        name: Option<impl AsRef<str>>,
        values: &[V],
    ) -> Field<Expr<V>, T> {
        assert_eq!(values.len(), index.length as usize);
        let buffer = device.create_buffer_from_slice(values);
        self.create_array_field_from_buffer(index, name, buffer)
    }
    pub fn create_array_field_from_buffer<V: Value>(
        &mut self,
        index: &ArrayIndex<T>,
        name: Option<impl AsRef<str>>,
        buffer: Buffer<V>,
    ) -> Field<Expr<V>, T> {
        assert_eq!(buffer.len(), index.length as usize);
        let field = self.create_field(name);
        let accessor = ArrayAccessor {
            index: index.clone(),
            buffer,
        };
        self.bind(field, accessor);
        field
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayIndex<T: EmanationType> {
    pub field: Field<Expr<u32>, T>,
    pub length: u32,
}

impl<T: EmanationType> IndexEmanation<Expr<u32>> for ArrayIndex<T> {
    type T = T;
    fn bind_fields(&self, idx: Expr<u32>, element: &mut Element<T>) {
        element.bind(self.field, ExprAccessor::new(idx));
    }
}
impl<T: EmanationType> IndexDomain for ArrayIndex<T> {
    type I = Expr<u32>;
    fn get_index(&self) -> Self::I {
        dispatch_id().x
    }
    fn dispatch_size(&self) -> [u32; 3] {
        [self.length, 1, 1]
    }
}

pub struct ArrayAccessor<V: Value, T: EmanationType> {
    pub index: ArrayIndex<T>,
    pub buffer: Buffer<V>,
}
impl<V: Value, T: EmanationType> Accessor<T> for ArrayAccessor<V, T> {
    type V = Expr<V>;
    type C = SimpleCache<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        if let Some(cache) = self.get_cache(element, field) {
            Ok(cache.var.load())
        } else {
            let value = self.buffer.read(element.get(self.index.field));
            self.insert_cache(element, field, SimpleCache { var: value.var() });
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
            cache.var.store(value.clone());
        } else {
            self.insert_cache(element, field, SimpleCache { var: value.var() });
        }
        Ok(())
    }

    fn save(&self, element: &Element<T>, field: Field<Self::V, T>) {
        self.buffer.write(
            element.get(self.index.field),
            self.get_cache(element, field).unwrap().var.load(),
        );
    }

    fn can_write(&self) -> bool {
        true
    }
}
