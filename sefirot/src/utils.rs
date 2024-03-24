use std::ops::Deref;

use luisa_compute::lang::types::AtomicRef;

use crate::graph::{CopyExt, NodeConfigs};
use crate::luisa::prelude::*;

/// A struct that runs a given function upon drop.
#[derive(Debug, Clone)]
pub struct FnRelease<F: FnOnce() + 'static>(Option<F>);
impl<F: FnOnce() + 'static> FnRelease<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}
impl<F: FnOnce() + 'static> Drop for FnRelease<F> {
    fn drop(&mut self) {
        self.0.take().unwrap()();
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Paradox {}

pub struct Singleton<V: Value>(pub Buffer<V>);
impl<V: Value> Deref for Singleton<V> {
    type Target = SingletonVar<V>;
    fn deref(&self) -> &Self::Target {
        #[allow(clippy::needless_lifetimes)]
        unsafe fn cast_ptr<'a, T, S>(x: &'a T) -> &'a S {
            &*(x as *const T as *const S)
        }
        let buffer_var = &**self.0;
        unsafe { cast_ptr::<BufferVar<V>, SingletonVar<V>>(buffer_var) }
    }
}
impl<V: Value> Singleton<V> {
    pub fn new(device: &Device) -> Self {
        Self(device.create_buffer::<V>(1))
    }
    pub fn write_host(&self, value: V) -> NodeConfigs<'static>
    where
        V: Send,
    {
        self.0.copy_from_vec(vec![value])
    }
}

#[repr(transparent)]
pub struct SingletonVar<V: Value>(pub BufferVar<V>);
impl<V: Value> SingletonVar<V> {
    pub fn read(&self) -> Expr<V> {
        self.0.read(0)
    }
    pub fn write(&self, value: Expr<V>) {
        self.0.write(0, value)
    }
    pub fn atomic(&self) -> AtomicRef<V> {
        self.0.atomic_ref(0)
    }
}
