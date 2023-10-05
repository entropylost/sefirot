use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use generational_arena::{Arena, Index};

use crate::accessor::DynAccessor;
use crate::prelude::*;

static NEXT_EMANATION_ID: AtomicU64 = AtomicU64::new(0);

// States what the original ID is; eg: Particles for example.
pub trait EmanationType: Sync + Send + Debug + Copy + Eq + 'static {}

#[derive(Clone, Copy)]
pub struct Field<V: Any, T: EmanationType> {
    pub(crate) raw: RawFieldHandle,
    pub(crate) emanation_id: u64,
    pub(crate) _marker: PhantomData<(V, T)>,
}
impl<V: Any, T: EmanationType> Debug for Field<V, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field").field("raw", &self.raw).finish()
    }
}
impl<V: Any, T: EmanationType> PartialEq for Field<V, T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.emanation_id == other.emanation_id
    }
}
impl<V: Any, T: EmanationType> Eq for Field<V, T> {}
impl<V: Any, T: EmanationType> Field<V, T> {
    pub fn from_raw(field: RawFieldHandle, id: u64) -> Self {
        Self {
            raw: field,
            emanation_id: id,
            _marker: PhantomData,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct RawFieldHandle(pub(crate) Index);
#[derive(Clone)]
pub(crate) struct RawField<T: EmanationType> {
    pub(crate) name: Option<String>,
    pub(crate) ty: TypeId,
    pub(crate) accessor: Option<Arc<dyn DynAccessor<T>>>,
}
impl<T: EmanationType> Debug for RawField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawField")
            .field("name", &self.name)
            .field("ty", &self.ty)
            .field("accessor", &self.accessor.as_ref().map(|_| "..."))
            .finish()
    }
}

#[derive(Debug)]
pub struct Emanation<T: EmanationType> {
    pub(crate) id: u64,
    pub(crate) fields: Arena<RawField<T>>,
}
impl<T: EmanationType> Clone for Emanation<T> {
    fn clone(&self) -> Self {
        Self {
            id: NEXT_EMANATION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            fields: self.fields.clone(),
        }
    }
}
impl<T: EmanationType> Emanation<T> {
    pub fn new() -> Self {
        Self {
            id: NEXT_EMANATION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            fields: Arena::new(),
        }
    }

    pub fn create_field<V: Any>(&mut self, name: Option<impl AsRef<str>>) -> Field<V, T> {
        let raw = RawFieldHandle(self.fields.insert(RawField {
            name: name.map(|x| x.as_ref().to_string()),
            ty: TypeId::of::<V>(),
            accessor: None,
        }));
        Field {
            raw,
            emanation_id: self.id,
            _marker: PhantomData,
        }
    }

    pub fn bind<V: Any>(
        &mut self,
        field: Field<V, T>,
        accessor: impl Accessor<T, V = V>,
    ) -> Arc<dyn DynAccessor<T>> {
        let a = Arc::new(accessor);
        self.fields[field.raw.0].accessor = Some(a.clone());
        a
    }
}
