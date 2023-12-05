use std::any::TypeId;
use std::hash::{BuildHasher, Hash, Hasher};

use ahash::random_state::RandomState;
use ahash::AHasher;

use super::*;

pub trait Tag: Debug + Eq + Hash + Copy + Clone + Send + Sync + 'static {}

pub struct DynTag<H: Hasher + 'static = AHasher> {
    tag: Box<dyn SafeTag<H>>,
}
impl<H: Hasher + 'static> DynTag<H> {
    pub fn new(tag: impl Tag) -> Self {
        Self { tag: Box::new(tag) }
    }
}
impl<H: Hasher + 'static> PartialEq for DynTag<H> {
    fn eq(&self, other: &Self) -> bool {
        self.tag.eq(other.tag.as_ref())
    }
}
impl<H: Hasher + 'static> Eq for DynTag<H> {}
impl<H: Hasher + 'static> Hash for DynTag<H> {
    fn hash<H1: Hasher>(&self, state: &mut H1) {
        // if TypeId::of::<H1>() == TypeId::of::<H>() {
        self.tag.hash(unsafe { std::mem::transmute(state) });
        // } else {
        //     unreachable!("Invalid hasher type.");
        // }
    }
}
impl<H: Hasher + 'static> Debug for DynTag<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.tag.fmt(f)
    }
}
impl<H: Hasher + 'static> Clone for DynTag<H> {
    fn clone(&self) -> Self {
        Self {
            tag: self.tag.clone_box(),
        }
    }
}

pub trait SafeTag<H: Hasher + 'static = AHasher>: Debug + Send + Sync + 'static {
    fn hash(&self, hasher: &mut H) -> u64;
    fn type_id(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn SafeTag<H>) -> bool;
    fn clone_box(&self) -> Box<dyn SafeTag<H>>;
}
impl<X, H: Hasher + 'static> SafeTag<H> for X
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
    fn eq(&self, other: &dyn SafeTag<H>) -> bool {
        other
            .as_any()
            .downcast_ref::<X>()
            .map_or(false, |x| x == self)
    }
    fn clone_box(&self) -> Box<dyn SafeTag<H>> {
        Box::new(*self)
    }
}

/// A bi-directional map from values to tags of arbitrary types.
pub struct TagMap<T: Clone + Eq + Hash, S: BuildHasher + 'static = RandomState> {
    tags: HashMap<T, DynTag<S::Hasher>, S>,
    values: HashMap<DynTag<S::Hasher>, T, S>,
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
            tags: HashMap::with_hasher(S::default()),
            values: HashMap::with_hasher(S::default()),
        }
    }
    pub fn insert_dyn(&mut self, tag: DynTag<S::Hasher>, value: T) -> bool {
        if self.tags.contains_key(&value) {
            return false;
        }
        self.tags.insert(value.clone(), tag.clone());
        self.values.insert(tag, value);
        true
    }
    /// Inserts a value with a given tag into the map, returning `false` and doing nothing if the value was already present.
    pub fn insert(&mut self, tag: impl Tag, value: T) -> bool {
        self.insert_dyn(DynTag::new(tag), value)
    }
    pub fn remove_dyn(&mut self, tag: DynTag<S::Hasher>) -> Option<T> {
        if let Some(value) = self.values.remove(&tag) {
            self.tags.remove(&value).unwrap();
            Some(value)
        } else {
            None
        }
    }
    /// Removes a value with a given tag from the map, returning the value removed.
    pub fn remove(&mut self, tag: impl Tag) -> Option<T> {
        self.remove_dyn(DynTag::new(tag))
    }
    /// Removes a value from the map, returning if the removal was sucessful.
    pub fn remove_value(&mut self, value: &T) -> Option<DynTag<S::Hasher>> {
        if let Some(tag) = self.tags.remove(value) {
            self.values.remove(&tag).unwrap();
            Some(tag)
        } else {
            None
        }
    }
    pub fn contains_value(&self, value: &T) -> bool {
        self.tags.contains_key(value)
    }
    pub fn contains_dyn(&self, tag: DynTag<S::Hasher>) -> bool {
        self.values.contains_key(&tag)
    }
    pub fn contains(&self, tag: impl Tag) -> bool {
        self.contains_dyn(DynTag::new(tag))
    }
    pub fn get_dyn(&self, tag: DynTag<S::Hasher>) -> Option<&T> {
        self.values.get(&tag)
    }
    pub fn get(&self, tag: impl Tag) -> Option<&T> {
        self.get_dyn(DynTag::new(tag))
    }
    pub fn get_tag(&self, value: &T) -> Option<DynTag<S::Hasher>> {
        self.tags.get(value).cloned()
    }
}
