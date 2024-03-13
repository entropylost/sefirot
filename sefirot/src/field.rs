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
impl<X: Access<Deref = AllowDeref>, T: EmanationType> Deref for Field<X, T> {
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

pub trait Access: 'static {
    type Downcast: Access;
    /// A marker to prevent an infinite-deref loop in [`Field`] due to the [`Paradox`] implementation.
    /// When implementing this trait, always use [`AllowDeref`] as the type.
    type Deref: AccessDerefType;

    /// The level of the access, used for dynamic dispatch.
    /// Do not implement this manually.
    fn level() -> AccessLevel {
        AccessLevel(Self::Downcast::level().0 + 1)
    }
}

impl<V: Value> Access for Expr<V> {
    type Downcast = Paradox;
    type Deref = AllowDeref;
}
impl<V: Value> Access for Var<V> {
    type Downcast = Expr<V>;
    type Deref = AllowDeref;
}
impl<V: Value> Access for AtomicRef<V> {
    type Downcast = Var<V>;
    type Deref = AllowDeref;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Static<T: 'static>(pub T);
impl<T: 'static> Access for Static<T> {
    type Downcast = Paradox;
    type Deref = AllowDeref;
}

mod access_deref {
    pub trait AccessDerefType {}
    pub enum AllowDeref {}
    pub enum BlockDeref {}
    impl AccessDerefType for AllowDeref {}
    impl AccessDerefType for BlockDeref {}
}
pub use access_deref::AllowDeref;
use access_deref::{AccessDerefType, BlockDeref};

use crate::mapping::DynMapping;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessLevel(pub(crate) u8);

impl Access for Paradox {
    type Downcast = Paradox;
    type Deref = BlockDeref;
    fn level() -> AccessLevel {
        AccessLevel(0)
    }
}
