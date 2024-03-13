use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;

use id_newtype::UniqueId;
use luisa::lang::types::AtomicRef;

use crate::emanation::EMANATIONS;
use crate::internal_prelude::*;
use crate::mapping::MappingBinding;

pub type EField<V, T> = Field<Expr<V>, T>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, UniqueId)]
pub struct FieldHandle {
    id: u64,
}

/// A single property of an [`Emanation`]. Note that by default, a `Field` has no actual data associated with it.
#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
#[repr(C)]
pub struct Field<X: Access, T: EmanationType> {
    pub(crate) handle: FieldHandle,
    pub(crate) emanation: EmanationHandle,
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

pub struct RawField {
    name: String,
}

pub(crate) struct Bindings(pub(crate) HashMap<FieldHandle, Box<dyn DynMapping>>);

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

impl<X: Access<Deref = AllowDeref>, T: EmanationType> Deref for Field<X, T> {
    type Target = Field<X::Downcast, T>;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const _ as *const Field<X::Downcast, T>) }
    }
}

impl<X: Access, T: EmanationType> Field<X, T> {
    pub fn at_opt(&self, index: &T::Index, ctx: &mut Context) -> Option<X> {
        if let Some(mapping) = ctx.bindings.0.remove(&self.handle) {
            let value = mapping.access_dyn(X::level(), index, ctx, self.handle);
            let value = *value.downcast().unwrap();
            ctx.bindings.0.insert(self.handle, mapping);
            Some(value)
        } else if let Some(emanation) = EMANATIONS.get(&self.emanation) {
            if let Some(mapping) = emanation.bindings.0.get(&self.handle) {
                let value = mapping.access_dyn(X::level(), index, ctx, self.handle);
                let value = *value.downcast().unwrap();
                Some(value)
            } else {
                None
            }
        } else {
            None
        }
    }
    pub fn at(&self, el: &mut Element<T::Index>) -> X {
        self.at_opt(&el.index, &mut el.context).unwrap()
    }
    pub fn bind(&self, mapping: impl Mapping<X, T::Index>) {
        EMANATIONS
            .get_mut(&self.emanation)
            .unwrap()
            .bindings
            .0
            .insert(
                self.handle,
                Box::new(MappingBinding::<X, T, _> {
                    mapping,
                    _marker: PhantomData,
                }),
            );
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
