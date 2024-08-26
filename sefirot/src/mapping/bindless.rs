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

// TODO: Make a `Var` version of this, wrapping the index probably.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
// The array stores a strong reference to the buffer so there's no need to hold it.
pub struct BindlessBufferHandle<V: Value> {
    pub index: u32,
    _marker: PhantomData<fn() -> V>,
}

impl<V: Value> BindlessHandle for BindlessBufferHandle<V> {
    type Dim = usize;
}

pub trait BindlessHandle {
    type Dim;
}

pub trait Emplace {
    type H: BindlessHandle;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::H;
    fn dim(&self) -> <Self::H as BindlessHandle>::Dim;
}

impl<V: Value> Emplace for Buffer<V> {
    type H = BindlessBufferHandle<V>;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::H {
        let index = mapper.next_buffer();
        mapper.array.emplace_buffer_async(index, &self);
        BindlessBufferHandle {
            index: index as u32,
            _marker: PhantomData,
        }
    }
    fn dim(&self) -> <Self::H as BindlessHandle>::Dim {
        self.len()
    }
}
impl<V: Value> Emplace for &Buffer<V> {
    type H = BindlessBufferHandle<V>;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::H {
        let index = mapper.next_buffer();
        mapper.array.emplace_buffer_async(index, self);
        BindlessBufferHandle {
            index: index as u32,
            _marker: PhantomData,
        }
    }
    fn dim(&self) -> <Self::H as BindlessHandle>::Dim {
        self.len()
    }
}
impl<V: Value> Emplace for &BufferView<V> {
    type H = BindlessBufferHandle<V>;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::H {
        let index = mapper.next_buffer();
        mapper.array.emplace_buffer_view_async(index, self);
        BindlessBufferHandle {
            index: index as u32,
            _marker: PhantomData,
        }
    }
    fn dim(&self) -> <Self::H as BindlessHandle>::Dim {
        self.len()
    }
}

// TODO: Add the dynamic version as well.. will require switching the field to Expr<u32> as well.
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
    pub fn new(size: usize) -> Self {
        let array = Arc::new(DEVICE.create_bindless_array(size));
        let (field, _handle) = Field::create_bind("bindless", BindlessArrayMapping(array.clone()));
        Self {
            array,
            field,
            _handle,
            free_buffers: Vec::new(),
            next_buffer: 0,
        }
    }
    pub fn next_buffer(&mut self) -> usize {
        self.free_buffers.pop().unwrap_or_else(|| {
            let index = self.next_buffer;
            self.next_buffer += 1;
            index
        })
    }
    pub fn emplace<T: Emplace>(&mut self, data: T) -> T::H {
        data.emplace_self(self)
    }
    pub fn emplace_blocking<T: Emplace>(&mut self, data: T) -> T::H {
        let index = data.emplace_self(self);
        self.update();
        index
    }
    pub fn update(&self) {
        self.array.update();
    }
    pub fn mapping<V: Value>(&self, handle: BindlessBufferHandle<V>) -> BindlessBufferMapping<V> {
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
    pub fn emplace_map<V: Value>(
        &mut self,
        data: impl Emplace<H = BindlessBufferHandle<V>>,
    ) -> BindlessBufferMapping<V> {
        let handle = self.emplace(data);
        self.mapping(handle)
    }
}
