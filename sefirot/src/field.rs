use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;

use dashmap::DashMap;
use id_newtype::UniqueId;
use luisa::lang::types::AtomicRef;
use once_cell::sync::Lazy;

use crate::internal_prelude::*;
use crate::mapping::MappingBinding;

pub(crate) static FIELDS: Lazy<DashMap<FieldHandle, RawField>> = Lazy::new(DashMap::new);

pub type EField<V, T> = Field<Expr<V>, T>;
pub type VField<V, T> = Field<Var<V>, T>;
pub type AField<V, T> = Field<AtomicRef<V>, T>;

#[derive(Copy, Clone, PartialEq, Eq, Hash, UniqueId)]
pub struct FieldHandle {
    id: u64,
}
impl Debug for FieldHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F{}", self.id)
    }
}

/// A single property of an [`Emanation`]. Note that by default, a `Field` has no actual data associated with it.
#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
#[repr(C)]
pub struct Field<X: Access, T: EmanationType> {
    pub(crate) handle: FieldHandle,
    pub(crate) emanation: EmanationId,
    pub(crate) _marker: PhantomData<(T, fn() -> X)>,
}
impl<X: Access, T: EmanationType> PartialEq for Field<X, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<X: Access, T: EmanationType> Eq for Field<X, T> {}
impl<X: Access, T: EmanationType> Clone for Field<X, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<X: Access, T: EmanationType> Copy for Field<X, T> {}
impl<X: Access, T: EmanationType> Deref for Field<X, T>
where
    X::Downcast: Access,
{
    type Target = Field<X::Downcast, T>;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const _ as *const Field<X::Downcast, T>) }
    }
}
impl<X: Access, T: EmanationType> Debug for Field<X, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!(
            "Field({})",
            FIELDS
                .get(&self.handle)
                .map_or_else(|| "dropped".to_string(), |x| x.name.clone())
        ))
        .field("handle", &self.handle)
        .field("emanation", &self.emanation)
        .finish()
    }
}

impl<X: Access, T: EmanationType> Field<X, T> {
    pub fn at_opt(&self, index: &T::Index, ctx: &mut Context) -> Option<X> {
        if let Some(mapping) = ctx.bindings.remove(&self.handle) {
            let value = mapping.access_dyn(X::level(), index, ctx, self.handle);
            let value = *value.downcast().unwrap();
            ctx.bindings.insert(self.handle, mapping);
            Some(value)
        } else if let Some(mapping) = FIELDS
            .get(&self.handle)
            .expect("Field dropped")
            .binding
            .as_ref()
        {
            let value = mapping.access_dyn(X::level(), index, ctx, self.handle);
            let value = *value.downcast().unwrap();
            Some(value)
        } else {
            None
        }
    }
    pub fn at(&self, el: &mut Element<T::Index>) -> X {
        self.at_opt(&el.index, &mut el.context).unwrap()
    }
    pub fn bind(&self, mapping: impl Mapping<X, T::Index>) {
        *FIELDS
            .get_mut(&self.handle)
            .expect("Field dropped")
            .binding
            .as_mut()
            .unwrap() = Box::new(MappingBinding::<X, T, _>::new(mapping));
    }
    pub fn emanation(&self) -> EmanationId {
        self.emanation
    }
    pub fn name(&self) -> String {
        FIELDS
            .get(&self.handle)
            .expect("Field dropped")
            .name
            .clone()
    }
}
impl<V: Value, T: EmanationType> Field<Expr<V>, T> {
    pub fn get(&self, el: &mut Element<T::Index>) -> Expr<V> {
        self.at(el)
    }
}
impl<V: Value, T: EmanationType> Field<Var<V>, T> {
    pub fn get_mut(&self, el: &mut Element<T::Index>) -> Var<V> {
        self.at(el)
    }
}
impl<V: Value, T: EmanationType> Field<AtomicRef<V>, T> {
    pub fn get_atomic(&self, el: &mut Element<T::Index>) -> AtomicRef<V> {
        self.at(el)
    }
}

pub struct RawField {
    pub(crate) name: String,
    pub(crate) binding: Option<Box<dyn DynMapping>>,
}

pub struct AccessCons<X: Access, L: AccessList>(PhantomData<fn() -> (X, L)>);
pub struct AccessNil;
pub trait AccessList {
    type Head;
    type Tail: AccessList;
}

impl AccessList for AccessNil {
    type Head = Paradox;
    type Tail = AccessNil;
}
impl<X: Access, L: AccessList> AccessList for AccessCons<X, L> {
    type Head = X;
    type Tail = L;
}

pub trait ListAccess {
    type List: AccessList;
    fn level() -> AccessLevel;
}

pub trait Access: ListAccess + 'static {
    type Downcast: ListAccess;
}

impl ListAccess for Paradox {
    type List = AccessNil;
    fn level() -> AccessLevel {
        AccessLevel(0)
    }
}
impl<X: Access> ListAccess for X {
    type List = AccessCons<X, <X::Downcast as ListAccess>::List>;
    fn level() -> AccessLevel {
        AccessLevel(X::Downcast::level().0 + 1)
    }
}

impl<V: Value> Access for Expr<V> {
    type Downcast = Paradox;
}
impl<V: Value> Access for Var<V> {
    type Downcast = Expr<V>;
}
impl<V: Value> Access for AtomicRef<V> {
    type Downcast = Var<V>;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Static<T: 'static>(pub T);
impl<T: 'static> Access for Static<T> {
    type Downcast = Paradox;
}

use crate::mapping::DynMapping;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessLevel(pub(crate) u8);
