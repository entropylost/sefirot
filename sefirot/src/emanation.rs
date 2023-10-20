use std::any::Any;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use generational_arena::{Arena, Index};
use parking_lot::Mutex;

use crate::field::DynAccessor;
use crate::prelude::*;

static NEXT_EMANATION_ID: AtomicU64 = AtomicU64::new(0);

// States what the original ID is; eg: Particles for example.
pub trait EmanationType: Sync + Send + Debug + Copy + 'static {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct RawFieldHandle(pub(crate) Index);
#[derive(Clone)]
pub(crate) struct RawField<T: EmanationType> {
    pub(crate) name: String,
    pub(crate) ty_name: String,
    pub(crate) accessor: Option<Arc<dyn DynAccessor<T>>>,
}
impl<T: EmanationType> Debug for RawField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("RawField<{}>", self.ty_name))
            .field("name", &self.name)
            .field(
                "accessor",
                &self.accessor.as_ref().map(|x| x.self_type_name()),
            )
            .finish()
    }
}

/// A structure representing a single format or space of data,
/// which might have a number of [`Field`]s associated with it.
///
/// Note that by default, a `Field` does not actually provide any data access mechanism.
/// In order to do that, it's necessary to bind an [`Accessor`].
/// This is done by most of the [`Emanation`] creation methods apart from [`create_field`].
#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
#[derive(Clone)]
pub struct Emanation<T: EmanationType> {
    pub(crate) id: u64,
    pub(crate) fields: Arc<Mutex<Arena<RawField<T>>>>,
    pub(crate) device: Device,
}
impl<T: EmanationType> Debug for Emanation<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut st = f.debug_struct(&format!("Emanation<{}>", std::any::type_name::<T>()));
        for (_, f) in self.fields.lock().iter() {
            st.field(
                &f.name,
                &format!(
                    "Field<{}>({})",
                    f.ty_name,
                    &f.accessor
                        .as_ref()
                        .map(|x| x.self_type_name())
                        .unwrap_or_else(|| "None".to_string())
                ),
            );
        }
        st.finish()
    }
}
impl<T: EmanationType> Emanation<T> {
    pub fn new(device: &Device) -> Self {
        Self {
            id: NEXT_EMANATION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            fields: Arc::new(Mutex::new(Arena::new())),
            device: device.clone(),
        }
    }

    pub fn create_field<V: Any>(&self, name: &str) -> Reference<'_, Field<V, T>> {
        let raw = RawFieldHandle(self.fields.lock().insert(RawField {
            name: name.to_string(),
            ty_name: std::any::type_name::<V>().to_string(),
            accessor: None,
        }));
        self.on(Field {
            raw,
            emanation_id: self.id,
            _marker: PhantomData,
        })
    }

    pub fn on<V: CanReference<T = T>>(&self, value: V) -> Reference<V> {
        Reference {
            emanation: self,
            value,
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }
}

/// A reference of an object within an [`Emanation`].
/// Used for [`Field`]-like things which are handles
/// and have no way of accessing data without the corrosponding [`Emanation`].
#[derive(Debug, Clone, Copy)]
pub struct Reference<'a, V: CanReference> {
    pub emanation: &'a Emanation<V::T>,
    pub value: V,
}
impl<V: CanReference> Deref for Reference<'_, V> {
    type Target = V;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<'a, V: CanReference> Reference<'a, V> {
    pub fn device(self) -> &'a Device {
        &self.emanation.device
    }
}
// TODO: Add name here.
// Also iadd trait for reference. Perhaps rename? Also index refs for `morton`.

pub trait CanReference: Copy {
    type T: EmanationType;
}
