use std::collections::HashSet;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Exclusive;

use id_newtype::UniqueId;
use pretty_type_name::pretty_type_name;

use crate::field::{RawField, FIELDS};
use crate::internal_prelude::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, UniqueId)]
pub struct EmanationId {
    id: u64,
}
impl Debug for EmanationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "E{}", self.id)
    }
}

pub trait EmanationType: Sync + Send + Debug + Copy + 'static {
    type Index: Clone + 'static;
}

// TODO: Debug impl.
pub struct Emanation<T: EmanationType> {
    pub(crate) device: Device,
    pub(crate) id: EmanationId,
    pub(crate) fields: HashSet<FieldHandle>,
    pub(crate) release: Vec<Exclusive<Box<dyn Send>>>,
    pub(crate) _marker: PhantomData<T>,
}
impl<T: EmanationType> Drop for Emanation<T> {
    fn drop(&mut self) {
        for field in self.fields.iter() {
            FIELDS.remove(field);
        }
    }
}
impl<T: EmanationType> Debug for Emanation<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct FieldsWrapper<'a>(&'a HashSet<FieldHandle>);
        impl<'a> Debug for FieldsWrapper<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_map()
                    .entries(self.0.iter().map(|x| {
                        (
                            x,
                            FIELDS
                                .get(x)
                                .map_or_else(|| "dropped".to_string(), |x| x.name.clone()),
                        )
                    }))
                    .finish()
            }
        }
        f.debug_struct(&format!("Emanation<{}>", pretty_type_name::<T>()))
            .field("id", &self.id)
            .field("fields", &FieldsWrapper(&self.fields))
            .finish()
    }
}
impl<T: EmanationType> Emanation<T> {
    pub fn id(&self) -> EmanationId {
        self.id
    }
    pub fn create_field<X: Access>(&self, name: impl AsRef<str>) -> Field<X, T> {
        let handle = FieldHandle::unique();
        FIELDS.insert(
            handle,
            RawField {
                name: name.as_ref().to_string(),
                binding: None,
            },
        );
        let emanation = self.id;
        Field {
            handle,
            emanation,
            _marker: PhantomData,
        }
    }
    /// Adds an object to be dropped when this emanation is dropped.
    pub fn release(&mut self, object: impl Send + 'static) {
        self.release.push(Exclusive::new(Box::new(object)));
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
