use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use crate::internal_prelude::*;
use crate::kernel::KernelContext;
use crate::mapping::{DynMapping, MappingBinding};

pub struct Element<I: Clone + 'static> {
    pub index: I,
    pub context: Context,
}
impl<I: Clone + 'static> Element<I> {
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
    pub(crate) bindings: HashMap<FieldHandle, Box<dyn DynMapping>>,
    pub cache: HashMap<FieldHandle, Box<dyn Any>>,
    pub kernel: Arc<KernelContext>,
    release: Vec<Box<dyn Any>>,
}
impl Context {
    pub fn new(kernel: Arc<KernelContext>) -> Self {
        Self {
            release: Vec::new(),
            bindings: HashMap::new(),
            cache: HashMap::new(),
            kernel,
        }
    }
    pub fn release(&mut self, object: impl Any) {
        self.release.push(Box::new(object));
    }
    pub fn bind_local<X: Access, T: EmanationType>(
        &mut self,
        field: Field<X, T>,
        mapping: impl Mapping<X, T::Index> + 'static,
    ) {
        let old = self.bindings.insert(
            field.handle,
            Box::new(MappingBinding::<X, T, _>::new(mapping)),
        );
        assert!(old.is_none(), "Field already bound");
    }
}
