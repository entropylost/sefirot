use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Exclusive;

use dashmap::DashMap;
use id_newtype::UniqueId;
use once_cell::sync::Lazy;
use pretty_type_name::pretty_type_name;

use crate::field::{Access, Bindings, FieldHandle, RawField};
use crate::prelude::*;
use crate::utils::Paradox;

// TODO: Find a way of doing this that won't have contention issues.
// TODO: Store fields globally instead? THen make Emanation contain device directly.
pub static EMANATIONS: Lazy<DashMap<EmanationHandle, RawEmanation>> = Lazy::new(DashMap::new);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, UniqueId)]
pub struct EmanationHandle {
    id: u64,
}

pub trait EmanationType: Sync + Send + Debug + Copy + 'static {
    type Index: Clone + 'static;
}

pub struct RawEmanation {
    // TODO: Name?
    pub(crate) bindings: Bindings,
    pub(crate) fields: HashMap<FieldHandle, RawField>,
    pub(crate) release: Vec<Exclusive<Box<dyn Send>>>,
    pub(crate) device: Device,
}

// TODO: Debug impl.
pub struct Emanation<T: EmanationType> {
    pub(crate) id: EmanationHandle,
    pub(crate) _marker: PhantomData<T>,
}
impl<T: EmanationType> Drop for Emanation<T> {
    fn drop(&mut self) {
        EMANATIONS.remove(&self.id);
    }
}
impl<T: EmanationType> Emanation<T> {
    pub fn create_field<X: Access>(&self) -> Field<X, T> {
        let handle = FieldHandle::unique();
        let emanation = self.id;
        Field {
            handle,
            emanation,
            _marker: PhantomData,
        }
    }
    /// Adds an object to be dropped when this emanation is dropped.
    pub fn release(&self, object: impl Send + 'static) {
        let mut map = EMANATIONS.get_mut(&self.id).unwrap();
        map.release.push(Exclusive::new(Box::new(object)));
    }
}

pub struct Auto<I: Clone + 'static> {
    _marker: PhantomData<fn() -> I>,
    _paradox: Paradox,
}
impl<I: Clone + 'static> Clone for Auto<I> {
    fn clone(&self) -> Self {
        #[allow(clippy::uninhabited_references)]
        *self
    }
}
impl<I: Clone + 'static> Copy for Auto<I> {}
impl<I: Clone + 'static> Debug for Auto<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Auto<{}>", pretty_type_name::<I>())
    }
}
impl<I: Clone + 'static> EmanationType for Auto<I> {
    type Index = I;
}
