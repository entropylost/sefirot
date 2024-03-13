use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Exclusive};

use static_assertions::assert_impl_all;

use crate::field::Bindings;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;

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
    pub bindings: Bindings,
    pub cache: HashMap<FieldHandle, Exclusive<Box<dyn Any + Send>>>,
    pub release: Vec<Exclusive<Box<dyn Send>>>,
    pub kernel: Arc<KernelContext>,
}
impl Context {
    pub fn new(kernel: Arc<KernelContext>) -> Self {
        Self {
            release: Vec::new(),
            bindings: Bindings(HashMap::new()),
            cache: HashMap::new(),
            kernel,
        }
    }
    pub fn release(&mut self, object: impl Send + 'static) {
        self.release.push(Exclusive::new(Box::new(object)));
    }
    pub fn cache(&mut self) -> &mut HashMap<FieldHandle, Exclusive<Box<dyn Any + Send>>> {
        &mut self.cache
    }
}

assert_impl_all!(Context: Send, Sync);
