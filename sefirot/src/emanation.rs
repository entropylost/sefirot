use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use generational_arena::{Arena, Index};
use parking_lot::Mutex;

use crate::field::{Accessor, DynAccessor};
use crate::prelude::*;

static NEXT_EMANATION_ID: AtomicU64 = AtomicU64::new(0);

// States what the original ID is; eg: Particles for example.
pub trait EmanationType: Sync + Send + Debug + Copy + Eq + 'static {}

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

#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
#[derive(Debug, Clone)]
pub struct Emanation<T: EmanationType> {
    pub(crate) id: u64,
    pub(crate) fields: Arc<Mutex<Arena<RawField<T>>>>,
}
impl<T: EmanationType> Emanation<T> {
    pub fn new() -> Self {
        Self {
            id: NEXT_EMANATION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            fields: Arc::new(Mutex::new(Arena::new())),
        }
    }

    pub fn create_field<V: Any>(&self, name: Option<&str>) -> Field<V, T> {
        let raw = RawFieldHandle(self.fields.lock().insert(RawField {
            name: name.map(|x| x.to_string()),
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
        &self,
        field: Field<V, T>,
        accessor: impl Accessor<T, V = V>,
    ) -> Arc<dyn DynAccessor<T>> {
        let a = Arc::new(accessor);
        self.fields.lock()[field.raw.0].accessor = Some(a.clone());
        a
    }

    pub fn create_bound_field<V: Any>(
        &self,
        name: Option<&str>,
        accessor: impl Accessor<T, V = V>,
    ) -> Field<V, T> {
        let field = self.create_field(name);
        self.bind(field, accessor);
        field
    }
    pub fn name_of<V: Any>(&self, field: Field<V, T>) -> Option<String> {
        self.fields.lock()[field.raw.0].name.clone()
    }
}
