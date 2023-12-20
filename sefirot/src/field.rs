use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use luisa::lang::types::AtomicRef;
use parking_lot::{MappedMutexGuard, MutexGuard};
use pretty_type_name::pretty_type_name;

use crate::emanation::{CanReference, RawFieldHandle, Reference};
use crate::prelude::*;

pub mod array;
pub mod constant;
pub mod map;
#[cfg(feature = "partition")]
pub mod partition;
pub mod slice;

pub struct FieldAccess<'a, V: Any, T: EmanationType> {
    el: &'a Element<T>,
    field: Field<V, T>,
    value: V,
    changed: bool,
}
impl<V: Any, T: EmanationType> FieldAccess<'_, V, T> {
    pub fn exists(&self) -> bool {
        self.el.has(self.field.raw)
    }
}
impl<V: Any, T: EmanationType> Deref for FieldAccess<'_, V, T> {
    type Target = V;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<V: Any, T: EmanationType> DerefMut for FieldAccess<'_, V, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.changed = true;
        &mut self.value
    }
}
impl<V: Any, T: EmanationType> Drop for FieldAccess<'_, V, T> {
    fn drop(&mut self) {
        if self.changed {
            self.el.set(self.field, &self.value).unwrap();
        }
    }
}

pub type EField<V, T> = Field<Expr<V>, T>;

/// A single property of an [`Emanation`]. Note that by default, a `Field` has no actual data associated with it.
#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
pub struct Field<V: Any, T: EmanationType> {
    pub(crate) raw: RawFieldHandle,
    pub(crate) emanation_id: u64,
    pub(crate) _marker: PhantomData<fn() -> (V, T)>,
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
        *self
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
    pub fn raw(&self) -> RawFieldHandle {
        self.raw
    }
    pub fn at<'a>(&self, el: &'a Element<T>) -> FieldAccess<'a, V, T> {
        let v = el.get(*self).unwrap();
        FieldAccess {
            el,
            field: *self,
            value: v,
            changed: false,
        }
    }
    #[doc(hidden)]
    pub fn __into_self(&self) -> Self {
        *self
    }
}
impl<T: EmanationType> Element<T> {
    #[doc(hidden)]
    pub fn __at<V: Any>(&self, field: Field<V, T>) -> FieldAccess<V, T> {
        field.at(self)
    }
}
impl<V: Any, T: EmanationType> CanReference for Field<V, T> {
    type T = T;
}
impl<'a, V: Any, T: EmanationType> Reference<'a, Field<V, T>> {
    /// Binds an accessor to a [`Field`], potentially allowing read and write access to it.
    pub fn bind(self, accessor: impl Accessor<T, V = V> + Send + Sync) -> Self {
        let a = &mut self.emanation.fields.write()[self.value.raw.0].accessor;
        if a.is_some() {
            panic!("Cannot bind accessor to already-bound field. If this is intentional, use `bind_override` instead.");
        }
        *a = Some(Arc::new(accessor));
        self
    }
    pub fn try_bind(self, accessor: impl Accessor<T, V = V> + Send + Sync) -> Result<Self, Self> {
        let a = &mut self.emanation.fields.write()[self.value.raw.0].accessor;
        if a.is_some() {
            Err(self)
        } else {
            *a = Some(Arc::new(accessor));
            Ok(self)
        }
    }
    pub fn bind_override(self, accessor: impl Accessor<T, V = V> + Send + Sync) -> Self {
        self.emanation.fields.write()[self.value.raw.0].accessor = Some(Arc::new(accessor));
        self
    }
    /// Binds an accessor to this field, returning the accessor. Equivalent to `.bind(accessor).accessor().unwrap()`.
    pub fn bind_accessor(
        self,
        accessor: impl Accessor<T, V = V> + Send + Sync,
    ) -> Arc<dyn DynAccessor<T> + Send + Sync> {
        let accessor = Arc::new(accessor);
        self.emanation.fields.write()[self.value.raw.0].accessor = Some(accessor.clone());
        accessor
    }
    pub fn accessor(self) -> Option<Arc<dyn DynAccessor<T> + Send + Sync>> {
        self.emanation.fields.read()[self.value.raw.0]
            .accessor
            .clone()
    }
    pub fn exists(self) -> bool {
        self.emanation.fields.read()[self.value.raw.0]
            .accessor
            .is_some()
    }
    pub fn can_write(self) -> bool {
        self.accessor().map_or(false, |x| x.can_write())
    }
    pub fn name(self) -> String {
        self.emanation.fields.read()[self.value.raw.0].name.clone()
    }
    pub fn named(self, name: &str) -> Self {
        self.emanation.fields.write()[self.value.raw.0].name = name.to_string();
        self
    }

    pub fn bind_fn(self, f: impl Fn(&Element<T>) -> V + Send + Sync + 'static) -> Self
    where
        V: Clone,
    {
        self.bind(FnAccessor::new(f))
    }
    pub fn bind_value(self, v: V) -> Self
    where
        V: Clone + Send + Sync,
    {
        self.bind(ValueAccessor(v))
    }

    pub fn map<W: Clone + Any>(
        self,
        f: impl Fn(V, &Element<T>) -> W + Send + Sync + 'static,
    ) -> Reference<'a, Field<W, T>> {
        self.emanation
            .create_field(&format!(
                "{}-mapped({} -> {})",
                self.name(),
                pretty_type_name::<V>(),
                pretty_type_name::<W>()
            ))
            .bind_fn(move |el| f(el.get(self.value).unwrap(), el))
    }
}

impl<'a, V: Value, T: EmanationType> Reference<'a, EField<V, T>> {
    /// Creates a [`Field`] that can be used to perform atomic operations on the values of this [`Field`].
    /// Panics if this [`Field`] is not bound to an [`Accessor`] with an implemented [`Accessor::get_atomic`].
    /// The only implementations in this crate are [`BufferAccessor`] and [`StructArrayAccessor`].
    pub fn atomic(self) -> Reference<'a, Field<AtomicRef<V>, T>> {
        let name = self.name();
        let accessor = self.accessor().unwrap();
        let atomic = accessor.get_atomic(self.emanation).unwrap();
        self.emanation
            .on(Field::from_raw(atomic, self.value.emanation_id))
            .named(&format!("{}-atomic", name))
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

pub trait DynAccessor<T: EmanationType>: Any {
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
    fn self_type_name(&self) -> String;
    fn as_any(&self) -> &dyn Any;
    fn get_atomic(&self, emanation: &Emanation<T>) -> Option<RawFieldHandle>;
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
    fn self_type_name(&self) -> String {
        pretty_type_name::<X>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn get_atomic(&self, emanation: &Emanation<T>) -> Option<RawFieldHandle> {
        Accessor::get_atomic(self, emanation)
    }
}
impl<T: EmanationType> Debug for dyn DynAccessor<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!(
            "DynAccessor<{}, V = {}>",
            pretty_type_name::<T>(),
            self.value_type_name()
        ))
        .field(&self.self_type_name())
        .finish()
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
        element
            .cache
            .try_lock()
            .unwrap()
            .insert(field.raw, Box::new(value));
    }
    fn get_cache<'a>(
        &'a self,
        element: &'a Element<T>,
        field: Field<Self::V, T>,
    ) -> Option<MappedMutexGuard<'a, Self::C>> {
        MutexGuard::try_map(element.cache.try_lock().unwrap(), |x| {
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
        if let Some(x) = self.get_cache(element, field) {
            x
        } else {
            let res = f();
            element
                .cache
                .try_lock()
                .unwrap()
                .insert(field.raw, Box::new(res));
            self.get_cache(element, field).unwrap()
        }
    }
    fn can_write(&self) -> bool;
    fn get_atomic(&self, _emanation: &Emanation<T>) -> Option<RawFieldHandle> {
        None
    }
}

pub struct ValueAccessor<V: Clone + Any>(pub V);
impl<V: Clone + Any, T: EmanationType> Accessor<T> for ValueAccessor<V> {
    type V = V;
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
            message: "Cannot write to `ValueAccessor` field".to_string(),
        })
    }
    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {
        unreachable!();
    }
    fn can_write(&self) -> bool {
        false
    }
}

pub struct FnAccessor<V: Clone + Any, F: Fn(&Element<T>) -> V + 'static, T: EmanationType> {
    f: F,
    _marker: PhantomData<(fn() -> V, T)>,
}
impl<V: Clone + Any, F: Fn(&Element<T>) -> V + 'static, T: EmanationType> FnAccessor<V, F, T> {
    pub fn new(f: F) -> Self {
        Self {
            f,
            _marker: PhantomData,
        }
    }
}
impl<V: Clone + Any, F: Fn(&Element<T>) -> V + 'static, T: EmanationType> Accessor<T>
    for FnAccessor<V, F, T>
{
    type V = V;
    type C = V;
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
            message: "Cannot write to `FnAccessor` field".to_string(),
        })
    }
    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {
        unreachable!();
    }
    fn can_write(&self) -> bool {
        false
    }
}
