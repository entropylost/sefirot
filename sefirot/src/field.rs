use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use parking_lot::{MappedMutexGuard, MutexGuard};
use pretty_type_name::pretty_type_name;

use crate::emanation::RawFieldHandle;
use crate::prelude::*;

pub mod array;
pub mod constant;

pub struct FieldAccess<'a: 'b, 'b, V: Any, T: EmanationType> {
    el: &'b Element<'a, T>,
    field: Field<V, T>,
    value: V,
    changed: bool,
}
impl<V: Any, T: EmanationType> Deref for FieldAccess<'_, '_, V, T> {
    type Target = V;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<V: Any, T: EmanationType> DerefMut for FieldAccess<'_, '_, V, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.changed = true;
        &mut self.value
    }
}
impl<V: Any, T: EmanationType> Drop for FieldAccess<'_, '_, V, T> {
    fn drop(&mut self) {
        if self.changed {
            self.el.set(self.field, &self.value);
        }
    }
}

#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
pub struct Field<V: Any, T: EmanationType> {
    pub(crate) raw: RawFieldHandle,
    pub(crate) emanation_id: u64,
    pub(crate) _marker: PhantomData<(V, T)>,
}
impl<V: Any, T: EmanationType> Debug for Field<V, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("raw", &self.raw)
            .field("emanation_id", &self.emanation_id)
            .finish()
    }
}
impl<V: Any, T: EmanationType> PartialEq for Field<V, T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.emanation_id == other.emanation_id
    }
}
impl<V: Any, T: EmanationType> Eq for Field<V, T> {}
impl<V: Any, T: EmanationType> Clone for Field<V, T> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw,
            emanation_id: self.emanation_id,
            _marker: PhantomData,
        }
    }
}
impl<V: Any, T: EmanationType> Copy for Field<V, T> {}

impl<V: Any, T: EmanationType> Field<V, T> {
    pub fn from_raw(field: RawFieldHandle, id: u64) -> Self {
        Self {
            raw: field,
            emanation_id: id,
            _marker: PhantomData,
        }
    }
    pub fn at<'a: 'b, 'b>(&self, el: &'b Element<'a, T>) -> FieldAccess<'a, 'b, V, T> {
        let v = el.get(*self);
        FieldAccess {
            el,
            field: *self,
            value: v,
            changed: false,
        }
    }
}

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

pub struct ExprFnAccessor<V: Value, F: Fn(&Element<T>) -> Expr<V> + 'static, T: EmanationType> {
    f: F,
    _marker: PhantomData<(V, T)>,
}
impl<V: Value, F: Fn(&Element<T>) -> Expr<V> + 'static, T: EmanationType> ExprFnAccessor<V, F, T> {
    pub fn new(f: F) -> Self {
        Self {
            f,
            _marker: PhantomData,
        }
    }
}
impl<V: Value, F: Fn(&Element<T>) -> Expr<V> + 'static, T: EmanationType> Accessor<T>
    for ExprFnAccessor<V, F, T>
{
    type V = Expr<V>;
    type C = Expr<V>;
    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        Ok(self
            .get_or_insert_cache(element, field, || (self.f)(element))
            .clone())
    }
    fn set(
        &self,
        _element: &Element<T>,
        _field: Field<Self::V, T>,
        _value: &Self::V,
    ) -> Result<(), WriteError> {
        Err(WriteError {
            message: "Cannot write to `ExprFnAccessor`".to_string(),
        })
    }
    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {}
    fn can_write(&self) -> bool {
        false
    }
}
