use std::sync::Arc;

use crate::ext_prelude::*;
use crate::field::{FieldHandle, Static};

pub struct BindlessMapper {
    array: Arc<BindlessArray>,
    field: SField<BindlessArrayVar, ()>,
    _handle: FieldHandle,
    free_buffers: Vec<usize>,
    free_tex2ds: Vec<usize>,
    free_tex3ds: Vec<usize>,
    next_buffer: usize,
    next_tex2d: usize,
    next_tex3d: usize,
}

#[derive(Value, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct BindlessBuffer {
    index: u32,
}

#[derive(Value, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct BindlessTex2d {
    index: u32,
}

#[derive(Value, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct BindlessTex3d {
    index: u32,
}

pub trait Emplace {
    type Index;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::Index;
}

pub trait Remove {
    fn remove_self(self, mapper: &mut BindlessMapper);
}

impl<V: Value> Emplace for &Buffer<V> {
    type Index = BindlessBuffer;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::Index {
        let buffer = mapper.next_buffer();
        mapper
            .array
            .emplace_buffer_async(buffer.index as usize, self);
        buffer
    }
}
impl<V: Value> Emplace for &BufferView<V> {
    type Index = BindlessBuffer;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::Index {
        let buffer = mapper.next_buffer();
        mapper
            .array
            .emplace_buffer_view_async(buffer.index as usize, self);
        buffer
    }
}
impl<V: IoTexel> Emplace for &Tex2d<V> {
    type Index = BindlessTex2d;
    fn emplace_self(self, mapper: &mut BindlessMapper) -> Self::Index {
        let texture = mapper.next_tex2d();
        mapper.array.emplace_tex2d_async(
            texture.index as usize,
            self,
            Sampler {
                filter: SamplerFilter::Point,
                address: SamplerAddress::Repeat,
            },
        );
        texture
    }
}

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
            free_tex2ds: Vec::new(),
            free_tex3ds: Vec::new(),
            next_buffer: 0,
            next_tex2d: 0,
            next_tex3d: 0,
        }
    }
    pub fn next_buffer(&mut self) -> BindlessBuffer {
        let index = self.free_buffers.pop().unwrap_or_else(|| {
            let index = self.next_buffer;
            self.next_buffer += 1;
            index
        }) as u32;
        BindlessBuffer { index }
    }
    pub fn next_tex2d(&mut self) -> BindlessTex2d {
        let index = self.free_tex2ds.pop().unwrap_or_else(|| {
            let index = self.next_tex2d;
            self.next_tex2d += 1;
            index
        }) as u32;
        BindlessTex2d { index }
    }
    pub fn next_tex3d(&mut self) -> BindlessTex3d {
        let index = self.free_tex3ds.pop().unwrap_or_else(|| {
            let index = self.next_tex3d;
            self.next_tex3d += 1;
            index
        }) as u32;
        BindlessTex3d { index }
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
}
