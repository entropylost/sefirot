use std::any::TypeId;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use dashmap::DashMap;
use id_newtype::UniqueId;
use luisa::lang::types::AtomicRef;
use once_cell::sync::Lazy;
use pretty_type_name::pretty_type_name;

use crate::internal_prelude::*;
use crate::mapping::function::FieldMapping;
use crate::mapping::{DynMapping, MappingBinding};

pub mod access;
pub use access::Access;

use self::access::{AccessCons, AccessNil};

pub mod set;

pub(crate) static FIELDS: Lazy<DashMap<FieldId, RawField>> = Lazy::new(DashMap::new);

pub type EEField<V, I> = Field<Expr<V>, Expr<I>>;
pub type VEField<V, I> = Field<Var<V>, Expr<I>>;
pub type AEField<V, I> = Field<AtomicRef<V>, Expr<I>>;
pub type SEField<V, I> = Field<Static<V>, Expr<I>>;
pub type EField<V, I> = Field<Expr<V>, I>;
pub type VField<V, I> = Field<Var<V>, I>;
pub type AField<V, I> = Field<AtomicRef<V>, I>;
pub type SField<V, I> = Field<Static<V>, I>;

pub trait FieldIndex: Clone + 'static {}
impl<T: Clone + 'static> FieldIndex for T {}

#[derive(PartialEq, Eq, Hash)]
pub struct FieldHandle(FieldId);
impl Debug for FieldHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RF{}", self.id)
    }
}
impl Deref for FieldHandle {
    type Target = FieldId;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for FieldHandle {
    fn drop(&mut self) {
        FIELDS.remove(&self.0);
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, UniqueId)]
pub struct FieldId {
    id: u64,
}
impl Debug for FieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F{}", self.id)
    }
}
impl FieldId {
    pub fn field_desc(self) -> Option<String> {
        if let Some(raw) = FIELDS.get(&self) {
            Some(format!(
                "Field<{}, {}>({})",
                raw.access_type_name, raw.index_type_name, raw.name
            ))
        } else {
            None
        }
    }
    pub fn get_typed<X: Access, I: FieldIndex>(self) -> Option<Field<X, I>> {
        let raw = FIELDS.get(&self)?;
        if raw.access_types.contains(&TypeId::of::<X>()) && raw.index_type == TypeId::of::<I>() {
            Some(Field {
                id: self,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
    pub fn as_typed<X: Access, I: FieldIndex>(self) -> Field<X, I> {
        self.get_typed().unwrap()
    }
    pub fn get_at<X: Access, I: FieldIndex>(self, index: &I, ctx: &mut Context) -> Option<X> {
        ctx.context_stack
            .last_mut()
            .unwrap()
            .entry(self)
            .or_default()
            .insert(X::level());
        ctx.on_mapping_opt(self, |ctx, mapping| {
            if let Some(mapping) = mapping {
                let value = mapping.access_dyn(X::level(), index, ctx, FieldBinding::new(self));
                let value = *value.downcast().unwrap();
                Some(value)
            } else {
                None
            }
        })
    }
}

/// A single property of an [`Emanation`]. Note that by default, a `Field` has no actual data associated with it.
#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
#[repr(C)]
pub struct Field<X: Access, I: FieldIndex> {
    pub(crate) id: FieldId,
    pub(crate) _marker: PhantomData<fn() -> (I, X)>,
}
impl<X: Access, I: FieldIndex> PartialEq for Field<X, I> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
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
                .id
                .field_desc()
                .unwrap_or_else(|| "Field[dropped]".to_string()),
        )
        .field("handle", &self.id)
        .finish()
    }
}
impl<X: Access, T: FieldIndex> Hash for Field<X, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<X: Access, I: FieldIndex> Field<X, I> {
    pub fn at_split(&self, index: &I, ctx: &mut Context) -> X {
        self.id.get_at::<X, I>(index, ctx).unwrap()
    }
    pub fn at(&self, el: &Element<I>) -> X {
        self.at_split(el.index(), &mut el.context())
    }
    pub fn bind(&self, mapping: impl Mapping<X, I> + Send + Sync) -> Self {
        let binding = &mut FIELDS.get_mut(&self.id).expect("Field dropped").binding;
        debug_assert!(binding.is_none());
        *binding = Some(Box::new(MappingBinding::<X, I, _>::new(mapping)));
        *self
    }
    pub fn name(&self) -> String {
        FIELDS.get(&self.id).expect("Field dropped").name.clone()
    }
    pub fn id(&self) -> FieldId {
        self.id
    }
    /// Creates a new field with the given name, returning the field and a root handle, which will drop the field when dropped.
    pub fn create(name: impl AsRef<str>) -> (Self, FieldHandle) {
        let id = FieldId::unique();
        FIELDS.insert(
            id,
            RawField {
                name: name.as_ref().to_string(),
                access_type_name: pretty_type_name::<X>(),
                access_types: X::types(),
                index_type_name: pretty_type_name::<I>(),
                index_type: TypeId::of::<I>(),
                binding: None,
            },
        );
        (
            Field {
                id,
                _marker: PhantomData,
            },
            FieldHandle(id),
        )
    }
    pub fn create_bind(
        name: impl AsRef<str>,
        mapping: impl Mapping<X, I> + Send + Sync,
    ) -> (Self, FieldHandle) {
        let (field, handle) = Self::create(name);
        field.bind(mapping);
        (field, handle)
    }

    pub fn map<Y: Access<List = AccessCons<Y, AccessNil>>, F: Fn(X) -> Y + 'static>(
        &self,
        f: F,
    ) -> FieldMapping<X, Y, I, F> {
        FieldMapping {
            field: *self,
            f,
            _marker: PhantomData,
        }
    }
}
impl<X: Access> Field<X, ()> {
    pub fn at_global<I: FieldIndex>(&self, element: &Element<I>) -> X {
        self.at_split(&(), &mut element.context())
    }
}
impl<V: Value, I: FieldIndex> Field<Expr<V>, I> {
    pub fn expr(&self, el: &Element<I>) -> Expr<V> {
        self.at(el)
    }
}
impl<V: Value, I: FieldIndex> Field<Var<V>, I> {
    pub fn var(&self, el: &Element<I>) -> Var<V> {
        self.at(el)
    }
}
impl<V: Value, I: FieldIndex> Field<AtomicRef<V>, I> {
    pub fn atomic(&self, el: &Element<I>) -> AtomicRef<V> {
        self.at(el)
    }
}
impl<T: 'static, I: FieldIndex> Field<Static<T>, I> {
    pub fn static_(&self, el: &Element<I>) -> T {
        self.at(el).0
    }
}

pub struct RawField {
    pub(crate) name: String,
    pub(crate) access_type_name: String,
    pub(crate) access_types: Vec<TypeId>,
    pub(crate) index_type_name: String,
    pub(crate) index_type: TypeId,
    pub(crate) binding: Option<Box<dyn DynMapping + Send + Sync>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Static<T: 'static>(pub T);
impl<T: 'static> Access for Static<T> {
    type Downcast = Paradox;
}
impl<T: 'static> Deref for Static<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: 'static> DerefMut for Static<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<T: 'static> From<T> for Static<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
