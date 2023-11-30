use std::any::TypeId;
use std::hash::{BuildHasher, Hash, Hasher};

use ahash::random_state::RandomState;
use smallvec::SmallVec;

use super::*;

// The `Debug` is necessary to have good `Debug` impls for `TagMap`.
pub trait Tag: Debug + Eq + Hash + Copy + Clone + Send + Sync + 'static {}

pub trait DynTag<H: Hasher + 'static = ahash::AHasher>: Debug + Send + Sync + 'static {
    fn hash(&self, hasher: &mut H) -> u64;
    fn type_id(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn DynTag<H>) -> bool;
    fn clone_box(&self) -> Box<dyn DynTag<H>>;
}
impl<X, H: Hasher + 'static> DynTag<H> for X
where
    X: Tag,
{
    fn hash(&self, hasher: &mut H) -> u64 {
        self.hash(hasher);
        hasher.finish()
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<X>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq(&self, other: &dyn DynTag<H>) -> bool {
        other
            .as_any()
            .downcast_ref::<X>()
            .map_or(false, |x| x == self)
    }
    fn clone_box(&self) -> Box<dyn DynTag<H>> {
        Box::new(*self)
    }
}

/// A bi-directional map from values to tags of arbitrary types.
pub struct TagMap<T: Clone + Eq + Hash, S: BuildHasher + 'static = RandomState> {
    state: S,
    tags: HashMap<T, Box<dyn DynTag<S::Hasher>>, S>,
    reverse_tags: HashMap<(u64, TypeId), SmallVec<[(Box<dyn DynTag<S::Hasher>>, T); 1]>, S>,
}
impl<T: Debug + Clone + Eq + Hash, S: BuildHasher + 'static> Debug for TagMap<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut st = f.debug_struct("TagMap");
        for (value, tag) in &self.tags {
            st.field(&format!("{:?}", tag), value);
        }
        st.finish()
    }
}
impl<T: Clone + Eq + Hash, S: BuildHasher + Default + 'static> Default for TagMap<T, S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Clone + Eq + Hash, S: BuildHasher + 'static> TagMap<T, S> {
    /// Creates a new empty `TagMap` with the given hasher.
    pub fn new() -> Self
    where
        S: Default,
    {
        Self {
            state: S::default(),
            tags: HashMap::with_hasher(S::default()),
            reverse_tags: HashMap::with_hasher(S::default()),
        }
    }
    fn index(&self, tag: &dyn DynTag<S::Hasher>) -> (u64, TypeId) {
        (tag.hash(&mut self.state.build_hasher()), tag.type_id())
    }
    /// Inserts a value with a given tag into the map, returning `false` and doing nothing if the value was already present.
    pub fn insert(&mut self, tag: impl Tag, value: T) -> bool {
        let index = self.index(&tag);
        if self.tags.contains_key(&value)
            || self.reverse_tags.get(&index).map_or(false, |tags| {
                tags.iter()
                    .any(|(t, _)| DynTag::<S::Hasher>::eq(&tag, t.as_ref()))
            })
        {
            return false;
        }
        self.tags.insert(value.clone(), Box::new(tag));
        self.reverse_tags
            .entry(index)
            .or_default()
            .push((Box::new(tag), value));
        true
    }
    /// Removes a value with a given tag from the map, returning the value removed.
    pub fn remove_tag(&mut self, tag: impl Tag) -> Option<T> {
        let index = self.index(&tag);
        if let Some(tags) = self.reverse_tags.get_mut(&index) {
            let Some(index) = tags
                .iter()
                .position(|(t, _)| DynTag::<S::Hasher>::eq(&tag, t.as_ref()))
            else {
                return None;
            };
            let (_tag, value) = tags.swap_remove(index);
            self.tags.remove(&value).unwrap();
            Some(value)
        } else {
            None
        }
    }
    /// Removes a value from the map, returning if the removal was sucessful.
    pub fn remove_value(&mut self, value: &T) -> bool {
        let Some(tag) = self.tags.remove(value) else {
            return false;
        };
        let index = self.index(&*tag);
        let tags = self.reverse_tags.get_mut(&index).unwrap();
        tags.retain(|(_, v)| v != value);
        if tags.is_empty() {
            self.reverse_tags.remove(&index);
        }
        true
    }
    pub fn contains_value(&self, value: &T) -> bool {
        self.tags.contains_key(value)
    }
    pub fn contains_tag(&self, tag: impl Tag) -> bool {
        let index = self.index(&tag);
        self.reverse_tags.get(&index).map_or(false, |tags| {
            tags.iter()
                .any(|(t, _)| DynTag::<S::Hasher>::eq(&tag, t.as_ref()))
        })
    }
    pub fn get(&self, tag: impl Tag) -> Option<&T> {
        let index = self.index(&tag);
        self.reverse_tags
            .get(&index)
            .and_then(|tags| {
                tags.iter()
                    .find(|(t, _)| DynTag::<S::Hasher>::eq(&tag, t.as_ref()))
            })
            .map(|(_, v)| v)
    }
    pub fn get_tag(&self, value: &T) -> Option<&dyn DynTag<S::Hasher>> {
        self.tags.get(value).map(|x| x.as_ref())
    }
}
