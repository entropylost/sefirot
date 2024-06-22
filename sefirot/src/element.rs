use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::rc::{Rc, Weak};

use generational_arena::{Arena, Index};
use luisa::runtime::KernelArg;

use crate::field::access::AccessLevel;
use crate::field::FIELDS;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;
use crate::mapping::{DynMapping, MappingBinding};

pub trait AsKernelContext {
    fn as_kernel_context(&self) -> Rc<KernelContext>;
    fn at<I: FieldIndex>(&self, index: I) -> Element<I> {
        Element::new(index, Context::new(self.as_kernel_context()))
    }
    fn constant(&self) -> Element<()> {
        self.at(())
    }
    fn bind_arg<T: KernelArg>(&self, f: impl Fn() -> T + Send + 'static) -> T::Parameter {
        self.as_kernel_context().bind(f)
    }
    fn bind_arg_indirect<T: KernelArg, S: Deref<Target = T>>(
        &self,
        f: impl Fn() -> S + Send + 'static,
    ) -> T::Parameter {
        self.as_kernel_context().bind_indirect(f)
    }
}
impl AsKernelContext for Rc<KernelContext> {
    fn as_kernel_context(&self) -> Rc<KernelContext> {
        self.clone()
    }
}
impl<I: FieldIndex> AsKernelContext for Element<I> {
    fn as_kernel_context(&self) -> Rc<KernelContext> {
        self.context().kernel.clone()
    }
}
impl AsKernelContext for Context {
    fn as_kernel_context(&self) -> Rc<KernelContext> {
        self.kernel.clone()
    }
}

struct ActiveContext {
    index: Index,
}
impl Drop for ActiveContext {
    fn drop(&mut self) {
        ACTIVE_CONTEXTS.with(|active_contexts| {
            active_contexts.borrow_mut().contexts.remove(self.index);
        });
    }
}

#[derive(Clone)]
pub struct Element<I: FieldIndex> {
    index: I,
    active_context_index: Rc<ActiveContext>,
    context: Rc<RefCell<Context>>,
}
impl<I: FieldIndex> Element<I> {
    pub fn new(index: I, context: Context) -> Self {
        let context = Rc::new(RefCell::new(context));
        let active_context_index = ACTIVE_CONTEXTS.with(|active_contexts| {
            active_contexts
                .borrow_mut()
                .contexts
                .insert(Rc::downgrade(&context))
        });
        Self {
            index,
            context,
            active_context_index: Rc::new(ActiveContext {
                index: active_context_index,
            }),
        }
    }
    pub fn index(&self) -> &I {
        &self.index
    }
    pub fn context(&self) -> RefMut<'_, Context> {
        self.context.borrow_mut()
    }
    /// Changes the index of the Element, inheriting the context.
    pub fn with_index<J: FieldIndex>(&self, index: J) -> Element<J> {
        Element {
            index,
            context: self.context.clone(),
            active_context_index: self.active_context_index.clone(),
        }
    }

    /// Evaluates the field immediately.
    /// This returns nothing and is purely useful for performance reasons;
    /// some fields may cache data between invocations, and so in some cases it may be useful
    /// to have it evaluated earlier.
    /// For example:
    /// ```ignore
    /// if foo {
    ///     field.expr(&el);
    /// }
    /// field.expr(&el);
    /// ```
    /// would drop the field's cache after the foo block finishes and so require another fetch, but
    /// ```ignore
    /// el.resolve(&field);
    /// if foo {
    ///    field.expr(&el);
    /// }
    /// field.expr(&el);
    /// ```
    /// would not have that issue.
    pub fn resolve<X: Access>(&self, field: &Field<X, I>) {
        field.at(self);
    }
}
impl<I: FieldIndex> Deref for Element<I> {
    type Target = I;
    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

thread_local! {
    pub(crate) static ACTIVE_CONTEXTS: RefCell<ActiveContexts> = RefCell::new(ActiveContexts::default());
}

#[derive(Default)]
struct ActiveContexts {
    contexts: Arena<Weak<RefCell<Context>>>,
}

#[doc(hidden)]
pub fn __enter_block() {
    ACTIVE_CONTEXTS.with(|active_contexts| {
        for (_, context) in active_contexts.borrow().contexts.iter() {
            context.upgrade().unwrap().borrow_mut().enter();
        }
    });
}
#[doc(hidden)]
pub fn __exit_block() {
    ACTIVE_CONTEXTS.with(|active_contexts| {
        for (_, context) in active_contexts.borrow().contexts.iter() {
            context.upgrade().unwrap().borrow_mut().exit();
        }
    });
}
#[doc(hidden)]
pub fn __block<R>(f: impl Fn() -> R) -> impl Fn() -> R {
    move || {
        __enter_block();
        let result = f();
        __exit_block();
        result
    }
}
#[doc(hidden)]
pub fn __block_input<T, R>(f: impl Fn(T) -> R) -> impl Fn(T) -> R {
    move |x| {
        __enter_block();
        let result = f(x);
        __exit_block();
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldBinding {
    field: FieldId,
    index: u64,
    stack: Vec<u64>,
}
impl Deref for FieldBinding {
    type Target = FieldId;
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}
impl FieldBinding {
    pub fn next(&self) -> Self {
        Self {
            field: self.field,
            index: self.index + 1,
            stack: self.stack.clone(),
        }
    }
    pub fn push(&self, value: impl Into<u64>) -> Self {
        let mut stack = self.stack.clone();
        stack.push(self.index);
        stack.push(value.into());
        Self {
            field: self.field,
            index: 0,
            stack,
        }
    }
    pub(crate) fn new(field: FieldId) -> Self {
        Self {
            field,
            index: 0,
            stack: vec![],
        }
    }
}

struct FieldCache {
    cache_stack: Vec<HashMap<FieldBinding, Box<dyn Any>>>,
}
impl FieldCache {
    pub fn new() -> Self {
        Self {
            cache_stack: vec![HashMap::new()],
        }
    }
}

impl Default for FieldCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Context {
    pub(crate) bindings: HashMap<FieldId, Box<dyn DynMapping>>,
    // TODO: Find a way that doesn't cause field composers to have issues.
    cache: FieldCache,
    pub kernel: Rc<KernelContext>,
    pub(crate) context_stack: Vec<HashMap<FieldId, HashSet<AccessLevel>>>,
}
impl Context {
    pub fn new(kernel: Rc<KernelContext>) -> Self {
        Self {
            bindings: HashMap::new(),
            cache: FieldCache::new(),
            kernel,
            context_stack: vec![HashMap::new()],
        }
    }
    pub fn get_cache_global<X: 'static>(&mut self, key: &FieldBinding) -> Option<RefMut<X>> {
        RefMut::filter_map(self.kernel.global_cache.borrow_mut(), |cache| {
            cache.get_mut(key).map(|x| x.downcast_mut().unwrap())
        })
        .ok()
    }
    pub fn get_cache<X: 'static>(&mut self, key: &FieldBinding) -> Option<&mut X> {
        for cache in self.cache.cache_stack.iter_mut().rev() {
            if let Some(value) = cache.get_mut(key) {
                return Some(value.downcast_mut().unwrap());
            }
        }
        None
    }
    pub fn insert_cache<X: 'static>(&mut self, key: &FieldBinding, value: X) {
        self.cache
            .cache_stack
            .last_mut()
            .unwrap()
            .insert(key.clone(), Box::new(value));
    }
    pub fn insert_cache_global<X: 'static>(&mut self, key: &FieldBinding, value: X) {
        self.kernel
            .global_cache
            .borrow_mut()
            .insert(key.clone(), Box::new(value));
    }
    // TODO: Why can't this work the normal way?
    pub fn get_cache_or_insert_with<X: 'static, R>(
        &mut self,
        key: &FieldBinding,
        f: impl FnOnce(&mut Self) -> X,
        ret: impl FnOnce(&mut X) -> R,
    ) -> R {
        if let Some(value) = self.get_cache(key) {
            return ret(value);
        }
        let value = f(self);
        self.insert_cache(key, value);
        ret(self
            .cache
            .cache_stack
            .last_mut()
            .unwrap()
            .get_mut(key)
            .unwrap()
            .downcast_mut()
            .unwrap())
    }
    pub fn get_cache_or_insert_with_global<X: 'static, R>(
        &mut self,
        key: &FieldBinding,
        f: impl FnOnce(&mut Self) -> X,
        ret: impl FnOnce(&mut X) -> R,
    ) -> R {
        if let Some(mut value) = self.get_cache_global(key) {
            return ret(&mut value);
        }
        let value = f(self);
        self.insert_cache_global(key, value);
        ret(self
            .kernel
            .global_cache
            .borrow_mut()
            .get_mut(key)
            .unwrap()
            .downcast_mut()
            .unwrap())
    }
    pub(crate) fn enter(&mut self) {
        self.context_stack.push(HashMap::new());
        self.cache.cache_stack.push(HashMap::new());
    }
    pub(crate) fn exit(&mut self) {
        let last_context = self.context_stack.pop().unwrap();
        for (field, access) in last_context {
            let mut access = access.into_iter().collect::<Vec<_>>();
            access.sort_unstable();
            self.on_mapping(field, |ctx, mapping| {
                for i in access.into_iter().rev() {
                    mapping.save_dyn(i, ctx, FieldBinding::new(field));
                }
            });
        }
        self.cache.cache_stack.pop().unwrap();
    }
    pub fn bind_local<X: Access, I: FieldIndex>(
        &mut self,
        field: Field<X, I>,
        mapping: impl Mapping<X, I> + 'static,
    ) {
        let old = self
            .bindings
            .insert(field.id, Box::new(MappingBinding::<X, I, _>::new(mapping)));
        assert!(old.is_none(), "Field already bound");
    }
    pub fn on_mapping_opt<R>(
        &mut self,
        handle: FieldId,
        f: impl FnOnce(&mut Self, Option<&dyn DynMapping>) -> R,
    ) -> R {
        if let Some(mapping) = self.bindings.remove(&handle) {
            let result = f(self, Some(&*mapping));
            self.bindings.insert(handle, mapping);
            result
        } else if let Some(field) = FIELDS.get(&handle) {
            if let Some(mapping) = &field.binding {
                f(self, Some(&**mapping))
            } else {
                f(self, None)
            }
        } else {
            f(self, None)
        }
    }
    pub fn on_mapping<R>(
        &mut self,
        handle: FieldId,
        f: impl FnOnce(&mut Self, &dyn DynMapping) -> R,
    ) -> R {
        self.on_mapping_opt(handle, |ctx, mapping| {
            f(ctx, mapping.expect("Field not bound"))
        })
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        while !self.context_stack.is_empty() {
            self.exit();
        }
    }
}
