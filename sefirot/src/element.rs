use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;

use luisa_compute::runtime::{AsKernelArg, KernelArg, KernelArgEncoder, KernelBuilder};

use crate::emanation::RawFieldHandle;
use crate::field::{Accessor, DynAccessor};
use crate::prelude::*;

pub struct KernelContext<'a> {
    pub(crate) context: &'a Context,
    pub(crate) builder: Mutex<&'a mut KernelBuilder>,
}
impl KernelContext<'_> {
    pub fn bind<V: Value>(&self, access: impl Fn() -> V + 'static) -> Expr<V> {
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
    bindings: Mutex<Vec<Box<dyn Fn(&mut KernelArgEncoder)>>>,
}
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
impl Context {
    pub fn new() -> Self {
        Self {
            accessed_fields: Mutex::new(HashSet::new()),
            mutated_fields: Mutex::new(HashSet::new()),
            bindings: Mutex::new(Vec::new()),
        }
    }
}

pub struct Element<'a, T: EmanationType> {
    pub(crate) emanation: &'a Emanation<T>,
    pub(crate) overridden_accessors: Mutex<HashMap<RawFieldHandle, Arc<dyn DynAccessor<T>>>>,
    pub context: &'a KernelContext<'a>,
    pub cache: Mutex<HashMap<RawFieldHandle, Box<dyn Any>>>,
    pub unsaved_fields: Mutex<HashSet<RawFieldHandle>>,
}

impl<T: EmanationType> Element<'_, T> {
    fn get_accessor(&self, field: RawFieldHandle) -> Arc<dyn DynAccessor<T>> {
        if let Some(accessor) = self.overridden_accessors.lock().get(&field) {
            return accessor.clone();
        }
        self.emanation.fields[field.0]
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

    pub fn get<V: Any>(&self, field: Field<V, T>) -> V {
        let field = field.raw;
        self.context.context.accessed_fields.lock().insert(field);

        let accessor = self.get_accessor(field);
        *accessor.get(self, field).unwrap().downcast().unwrap()
    }

    pub fn set<V: Any>(&self, field: Field<V, T>, value: &V) {
        let field = field.raw;
        self.context.context.mutated_fields.lock().insert(field);

        let accessor = self.get_accessor(field);
        accessor.set(self, field, value).unwrap();

        self.unsaved_fields.lock().insert(field);
    }

    pub fn save(&self) {
        let unsaved_fields = self.unsaved_fields.lock().drain().collect::<Vec<_>>();
        for field in unsaved_fields {
            self.get_accessor(field).save(self, field);
        }
    }
}
impl<T: EmanationType> Drop for Element<'_, T> {
    fn drop(&mut self) {
        self.save();
    }
}
