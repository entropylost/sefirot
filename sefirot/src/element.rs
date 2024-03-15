use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use crate::field::access::AccessLevel;
use crate::field::FIELDS;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;
use crate::mapping::{DynMapping, MappingBinding};

pub struct Element<I: FieldIndex> {
    pub index: I,
    pub context: Context,
}
impl<I: FieldIndex> Element<I> {
    pub fn index(&self) -> I {
        self.index.clone()
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }
}

pub struct Context {
    pub(crate) bindings: HashMap<FieldId, Box<dyn DynMapping>>,
    pub cache: HashMap<FieldId, Box<dyn Any>>,
    pub kernel: Arc<KernelContext>,
    pub(crate) access_levels: HashMap<FieldId, AccessLevel>,
}
impl Context {
    pub fn new(kernel: Arc<KernelContext>) -> Self {
        Self {
            bindings: HashMap::new(),
            cache: HashMap::new(),
            access_levels: HashMap::new(),
            kernel,
        }
    }
    pub fn bind_local<X: Access, I: FieldIndex>(
        &mut self,
        field: Field<X, I>,
        mapping: impl Mapping<X, I> + 'static,
    ) {
        let old = self.bindings.insert(
            field.handle,
            Box::new(MappingBinding::<X, I, _>::new(mapping)),
        );
        assert!(old.is_none(), "Field already bound");
    }
    pub fn on_mapping_opt<R>(
        &mut self,
        handle: FieldId,
        f: impl FnOnce(&mut Self, Option<&dyn DynMapping>) -> R,
    ) -> R {
        if let Some(mapping) = self.bindings.remove(&handle) {
            let result = f(self, Some(&*mapping));
            self.bindings.insert(handle, mapping);
            result
        } else if let Some(field) = FIELDS.get(&handle) {
            if let Some(mapping) = &field.binding {
                f(self, Some(&**mapping))
            } else {
                f(self, None)
            }
        } else {
            f(self, None)
        }
    }
    pub fn on_mapping<R>(
        &mut self,
        handle: FieldId,
        f: impl FnOnce(&mut Self, &dyn DynMapping) -> R,
    ) -> R {
        self.on_mapping_opt(handle, |ctx, mapping| {
            f(ctx, mapping.expect("Field not bound"))
        })
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        let access_levels = std::mem::take(&mut self.access_levels);
        for (field, access) in access_levels.iter() {
            self.on_mapping(*field, |ctx, mapping| {
                for i in (1..=access.0).rev() {
                    mapping.save_dyn(AccessLevel(i), ctx, *field);
                }
            });
        }
    }
}
