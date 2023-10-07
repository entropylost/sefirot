use std::any::{Any, TypeId};
use std::marker::PhantomData;

use parking_lot::{MappedMutexGuard, MutexGuard};
use pretty_type_name::pretty_type_name;

use crate::emanation::RawFieldHandle;
use crate::prelude::*;

pub mod array;
pub mod constant;

#[derive(Debug, Clone, PartialEq)]
pub struct ReadError {
    message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WriteError {
    message: String,
}

pub trait DynAccessor<T: EmanationType> {
    fn get(&self, element: &Element<T>, field: RawFieldHandle) -> Result<Box<dyn Any>, ReadError>;
    fn set(
        &self,
        element: &Element<T>,
        field: RawFieldHandle,
        value: &dyn Any,
    ) -> Result<(), WriteError>;
    fn save(&self, element: &Element<T>, field: RawFieldHandle);
    fn can_write(&self) -> bool;
    fn value_type(&self) -> TypeId;
    fn value_type_name(&self) -> String;
    fn self_type(&self) -> TypeId;
    fn self_type_name(&self) -> String;
}
impl<X, T: EmanationType> DynAccessor<T> for X
where
    X: Accessor<T>,
{
    fn get(&self, element: &Element<T>, field: RawFieldHandle) -> Result<Box<dyn Any>, ReadError> {
        self.get(element, Field::from_raw(field, element.emanation.id))
            .map(|x| Box::new(x) as Box<dyn Any>)
    }
    fn set(
        &self,
        element: &Element<T>,
        field: RawFieldHandle,
        value: &dyn Any,
    ) -> Result<(), WriteError> {
        self.set(
            element,
            Field::from_raw(field, element.emanation.id),
            value.downcast_ref().unwrap(),
        )
    }
    fn save(&self, element: &Element<T>, field: RawFieldHandle) {
        Accessor::save(self, element, Field::from_raw(field, element.emanation.id))
    }
    fn can_write(&self) -> bool {
        Accessor::can_write(self)
    }
    fn value_type(&self) -> TypeId {
        TypeId::of::<X::V>()
    }
    fn value_type_name(&self) -> String {
        pretty_type_name::<X::V>()
    }
    fn self_type(&self) -> TypeId {
        TypeId::of::<X>()
    }
    fn self_type_name(&self) -> String {
        pretty_type_name::<X>()
    }
}

// Note that the Accessor has to be in charge of caching
// values between multiple runs.
pub trait Accessor<T: EmanationType>: 'static {
    type V: Any;
    type C: Any;
    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError>;
    fn set(
        &self,
        element: &Element<T>,
        field: Field<Self::V, T>,
        value: &Self::V,
    ) -> Result<(), WriteError>;
    fn save(&self, element: &Element<T>, field: Field<Self::V, T>);
    fn insert_cache(&self, element: &Element<T>, field: Field<Self::V, T>, value: Self::C) {
        element.cache.lock().insert(field.raw, Box::new(value));
    }
    fn get_cache<'a>(
        &'a self,
        element: &'a Element<T>,
        field: Field<Self::V, T>,
    ) -> Option<MappedMutexGuard<'a, Self::C>> {
        MutexGuard::try_map(element.cache.lock(), |x| {
            x.get_mut(&field.raw)
                .map(move |x| x.downcast_mut().unwrap())
        })
        .ok()
    }
    fn get_or_insert_cache<'a>(
        &'a self,
        element: &'a Element<T>,
        field: Field<Self::V, T>,
        f: impl FnOnce() -> Self::C,
    ) -> MappedMutexGuard<'a, Self::C> {
        MutexGuard::map(element.cache.lock(), |x| {
            x.entry(field.raw)
                .or_insert_with(|| Box::new(f()))
                .downcast_mut()
                .unwrap()
        })
    }
    fn can_write(&self) -> bool;
}

pub struct SimpleCache<V: Value> {
    pub var: Var<V>,
}

pub struct ExprAccessor<V: Value>(Expr<V>);
impl<V: Value> ExprAccessor<V> {
    pub fn new(expr: Expr<V>) -> Self {
        Self(expr)
    }
}
impl<V: Value, T: EmanationType> Accessor<T> for ExprAccessor<V> {
    type V = Expr<V>;
    type C = ();
    fn get(&self, _element: &Element<T>, _field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        Ok(self.0.clone())
    }
    fn set(
        &self,
        _element: &Element<T>,
        _field: Field<Self::V, T>,
        _value: &Self::V,
    ) -> Result<(), WriteError> {
        Err(WriteError {
            message: "Cannot write to `ExprAccessor`".to_string(),
        })
    }
    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {}
    fn can_write(&self) -> bool {
        false
    }
}
