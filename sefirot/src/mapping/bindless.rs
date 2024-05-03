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
}
