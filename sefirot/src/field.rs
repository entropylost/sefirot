use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;

use dashmap::DashMap;
use id_newtype::UniqueId;
use luisa::lang::types::AtomicRef;
use once_cell::sync::Lazy;

use crate::internal_prelude::*;
use crate::mapping::{DynMapping, MappingBinding};

pub mod access;
pub use access::Access;

pub mod set;

use self::access::AccessLevel;
use self::set::FieldSetId;

pub(crate) static FIELDS: Lazy<DashMap<FieldHandle, RawField>> = Lazy::new(DashMap::new);

pub type EField<V, T> = Field<Expr<V>, T>;
pub type VField<V, T> = Field<Var<V>, T>;
pub type AField<V, T> = Field<AtomicRef<V>, T>;

pub trait FieldIndex: Clone + 'static {}
impl<T: Clone + 'static> FieldIndex for T {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, UniqueId)]
pub struct FieldHandle {
    id: u64,
}
impl Debug for FieldHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F{}", self.id)
    }
}
impl FieldHandle {
    fn field_desc(&self) -> Option<String> {
        if let Some(raw) = FIELDS.get(self) {
            Some(format!(
                "Field<{}, {}>({})",
                raw.access_type_name, raw.index_type_name, raw.name
            ))
        } else {
            None
        }
    }
}

/// A single property of an [`Emanation`]. Note that by default, a `Field` has no actual data associated with it.
#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
#[repr(C)]
pub struct Field<X: Access, I: FieldIndex> {
    pub(crate) handle: FieldHandle,
    pub(crate) set: FieldSetId,
    pub(crate) _marker: PhantomData<fn() -> (I, X)>,
}
impl<X: Access, I: FieldIndex> PartialEq for Field<X, I> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<X: Access, I: FieldIndex> Eq for Field<X, I> {}
impl<X: Access, I: FieldIndex> Clone for Field<X, I> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<X: Access, I: FieldIndex> Copy for Field<X, I> {}
impl<X: Access, I: FieldIndex> Deref for Field<X, I>
where
    X::Downcast: Access,
{
    type Target = Field<X::Downcast, I>;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const _ as *const Field<X::Downcast, I>) }
    }
}
impl<X: Access, T: FieldIndex> Debug for Field<X, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(
            &self
                .handle
                .field_desc()
                .unwrap_or_else(|| "Field[dropped]".to_string()),
        )
        .field("handle", &self.handle)
        .field("set", &self.set)
        .finish()
    }
}

impl<X: Access, I: FieldIndex> Field<X, I> {
    pub fn at_opt(&self, index: &I, ctx: &mut Context) -> Option<X> {
        ctx.access_levels
            .entry(self.handle)
            .and_modify(|lvl| *lvl = AccessLevel(lvl.0.max(X::level().0)))
            .or_insert(X::level());
        ctx.on_mapping_opt(self.handle, |ctx, mapping| {
            if let Some(mapping) = mapping {
                let value = mapping.access_dyn(X::level(), index, ctx, self.handle);
                let value = *value.downcast().unwrap();
                Some(value)
            } else {
                None
            }
        })
    }
    pub fn at(&self, el: &mut Element<I>) -> X {
        self.at_opt(&el.index, &mut el.context).unwrap()
    }
    pub fn bind(&self, mapping: impl Mapping<X, I>) -> Self {
        *FIELDS
            .get_mut(&self.handle)
            .expect("Field dropped")
            .binding
            .as_mut()
            .unwrap() = Box::new(MappingBinding::<X, I, _>::new(mapping));
        *self
    }
    pub fn set(&self) -> FieldSetId {
        self.set
    }
    pub fn name(&self) -> String {
        FIELDS
            .get(&self.handle)
            .expect("Field dropped")
            .name
            .clone()
    }
}
impl<V: Value, I: FieldIndex> Field<Expr<V>, I> {
    pub fn get(&self, el: &mut Element<I>) -> Expr<V> {
        self.at(el)
    }
}
impl<V: Value, I: FieldIndex> Field<Var<V>, I> {
    pub fn get_mut(&self, el: &mut Element<I>) -> Var<V> {
        self.at(el)
    }
}
impl<V: Value, I: FieldIndex> Field<AtomicRef<V>, I> {
    pub fn get_atomic(&self, el: &mut Element<I>) -> AtomicRef<V> {
        self.at(el)
    }
}

pub struct RawField {
    pub(crate) name: String,
    pub(crate) access_type_name: String,
    pub(crate) index_type_name: String,
    pub(crate) binding: Option<Box<dyn DynMapping>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Static<T: 'static>(pub T);
impl<T: 'static> Access for Static<T> {
    type Downcast = Paradox;
}
