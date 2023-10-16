use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;
use static_assertions::assert_impl_all;

use luisa_compute::runtime::{AsKernelArg, KernelArg, KernelArgEncoder, KernelBuilder};

use crate::emanation::RawFieldHandle;
use crate::field::{Accessor, DynAccessor, ReadError, WriteError};
use crate::prelude::*;

#[derive(Clone)]
pub struct KernelContext {
    pub(crate) context: Arc<Context>,
    pub(crate) builder: Arc<Mutex<KernelBuilder>>,
}
impl KernelContext {
    pub fn bind<V: Value>(&self, access: impl Fn() -> V + Send + 'static) -> Expr<V> {
        let mut builder = self.builder.lock();
        self.context.bindings.lock().push(Box::new(move |encoder| {
            encoder.uniform(access());
        }));
        builder.uniform::<V>()
    }
}

pub struct Context {
    // TODO: Make this use domains.
    accessed_fields: Mutex<HashSet<RawFieldHandle>>,
    mutated_fields: Mutex<HashSet<RawFieldHandle>>,
    bindings: Mutex<Vec<Box<dyn Fn(&mut KernelArgEncoder) + Send>>>,
}
assert_impl_all!(Context: Send, Sync);
impl KernelArg for Context {
    type Parameter = ();
    fn encode(&self, encoder: &mut KernelArgEncoder) {
        for binding in self.bindings.lock().iter() {
            binding(encoder);
        }
    }
}
impl AsKernelArg for Context {
    type Output = Self;
}
impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
impl Context {
    pub fn new() -> Self {
        Self {
            accessed_fields: Mutex::new(HashSet::new()),
            mutated_fields: Mutex::new(HashSet::new()),
            bindings: Mutex::new(Vec::new()),
        }
    }
}

pub struct Element<T: EmanationType> {
    pub(crate) emanation: Emanation<T>,
    pub(crate) overridden_accessors: Mutex<HashMap<RawFieldHandle, Arc<dyn DynAccessor<T>>>>,
    pub context: KernelContext,
    pub cache: Mutex<HashMap<RawFieldHandle, Box<dyn Any>>>,
    pub unsaved_fields: Mutex<HashSet<RawFieldHandle>>,
}

impl<T: EmanationType> Element<T> {
    fn get_accessor(&self, field: RawFieldHandle) -> Arc<dyn DynAccessor<T>> {
        if let Some(accessor) = self.overridden_accessors.lock().get(&field) {
            return accessor.clone();
        }
        self.emanation.fields.lock()[field.0]
            .accessor
            .as_ref()
            .unwrap()
            .clone()
    }

    pub fn bind<V: Any>(&self, field: Field<V, T>, accessor: impl Accessor<T, V = V>) {
        self.overridden_accessors
            .lock()
            .insert(field.raw, Arc::new(accessor));
    }

    pub fn get<V: Any>(&self, field: Field<V, T>) -> Result<V, ReadError> {
        let field = field.raw;
        self.context.context.accessed_fields.lock().insert(field);

        let accessor = self.get_accessor(field);
        Ok(*accessor.get(self, field)?.downcast::<V>().unwrap())
    }

    pub fn set<V: Any>(&self, field: Field<V, T>, value: &V) -> Result<(), WriteError> {
        let field = field.raw;
        self.context.context.mutated_fields.lock().insert(field);

        let accessor = self.get_accessor(field);
        accessor.set(self, field, value)?;

        self.unsaved_fields.lock().insert(field);
        Ok(())
    }

    pub fn save(&self) {
        let unsaved_fields = self.unsaved_fields.lock().drain().collect::<Vec<_>>();
        for field in unsaved_fields {
            self.get_accessor(field).save(self, field);
        }
    }

    pub fn has(&self, field: RawFieldHandle) -> bool {
        self.overridden_accessors.lock().contains_key(&field)
            || self
                .emanation
                .fields
                .lock()
                .get(field.0)
                .and_then(|x| x.accessor.as_ref())
                .is_some()
    }
}
impl<T: EmanationType> Drop for Element<T> {
    fn drop(&mut self) {
        self.save();
    }
}
