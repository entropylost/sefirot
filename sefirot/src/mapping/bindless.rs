use std::marker::PhantomData;
use std::sync::Arc;

use super::cache::SimpleExprMapping;
use super::function::CachedFnMapping;
use crate::ext_prelude::*;
use crate::field::{FieldHandle, Static};
use crate::impl_cache_mapping;

pub struct BindlessMapper {
    array: Arc<BindlessArray>,
    field: SField<BindlessArrayVar, ()>,
    _handle: FieldHandle,
    free_buffers: Vec<usize>,
    next_buffer: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
// TODO: Make a `Var` version of this, wrapping the index probably.
pub struct BindlessBufferHandle<V: Value> {
    index: u32,
    _marker: PhantomData<fn() -> V>,
}

pub trait Emplace {
    type Index;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::Index;
}

pub trait Remove {
    fn remove_self(self, mapper: &mut BindlessMapper);
}

impl<V: Value> Emplace for &Buffer<V> {
    type Index = BindlessBufferHandle<V>;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::Index {
        let buffer = mapper.next_buffer();
        mapper
            .array
            .emplace_buffer_async(buffer.index as usize, self);
        buffer
    }
}
impl<V: Value> Emplace for &BufferView<V> {
    type Index = BindlessBufferHandle<V>;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::Index {
        let buffer = mapper.next_buffer();
        mapper
            .array
            .emplace_buffer_view_async(buffer.index as usize, self);
        buffer
    }
}

// TODO: Add the dynamic version as well.. will require switching the field to u32 as well?
pub struct BindlessBufferMapping<V: Value> {
    buffer_field: SField<BindlessBufferVar<V>, ()>,
    _buffer_field_handle: FieldHandle,
}

impl<V: Value> SimpleExprMapping<V, Expr<u32>> for BindlessBufferMapping<V> {
    fn get_expr(&self, index: &Expr<u32>, ctx: &mut Context) -> Expr<V> {
        self.buffer_field.at_split(&(), ctx).read(*index)
    }
    fn set_expr(&self, index: &Expr<u32>, value: Expr<V>, ctx: &mut Context) {
        self.buffer_field.at_split(&(), ctx).write(*index, value);
    }
}
impl_cache_mapping!([V: Value] Mapping[V, Expr<u32>] for BindlessBufferMapping<V>);

struct BindlessArrayMapping(Arc<BindlessArray>);
impl Mapping<Static<BindlessArrayVar>, ()> for BindlessArrayMapping {
    type Ext = ();
    fn access(
        &self,
        _index: &(),
        ctx: &mut Context,
        binding: FieldBinding,
    ) -> Static<BindlessArrayVar> {
        Static(ctx.get_cache_or_insert_with_global(
            &binding,
            |ctx| {
                let array = self.0.clone();
                ctx.bind_arg_indirect(move || array.clone())
            },
            |x| x.clone(),
        ))
    }
}

impl BindlessMapper {
    pub fn new(device: &Device, size: usize) -> Self {
        let array = Arc::new(device.create_bindless_array(size));
        let (field, _handle) = Field::create_bind("bindless", BindlessArrayMapping(array.clone()));
        Self {
            array,
            field,
            _handle,
            free_buffers: Vec::new(),
            next_buffer: 0,
        }
    }
    pub fn next_buffer<V: Value>(&mut self) -> BindlessBufferHandle<V> {
        let index = self.free_buffers.pop().unwrap_or_else(|| {
            let index = self.next_buffer;
            self.next_buffer += 1;
            index
        }) as u32;
        BindlessBufferHandle {
            index,
            _marker: PhantomData,
        }
    }
    pub fn emplace_async<T: Emplace>(&mut self, data: T) -> T::Index {
        data.emplace_self(self)
    }
    pub fn emplace<T: Emplace>(&mut self, data: T) -> T::Index {
        let index = data.emplace_self(self);
        self.update();
        index
    }
    pub fn update(&self) {
        self.array.update();
    }
    pub fn mappingd<V: Value>(&self, handle: BindlessBufferHandle<V>) -> BindlessBufferMapping<V> {
        let array_field = self.field;
        let (buffer_field, buffer_field_handle) = Field::create_bind(
            "bindless-buffer-field",
            CachedFnMapping::new(move |_, ctx| {
                let array = array_field.at_split(&(), ctx);
                Static(array.buffer(handle.index))
            }),
        );
        BindlessBufferMapping {
            buffer_field,
            _buffer_field_handle: buffer_field_handle,
        }
    }
}
