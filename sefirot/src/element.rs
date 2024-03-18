use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use generational_arena::{Arena, Index};

use crate::field::access::AccessLevel;
use crate::field::FIELDS;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;
use crate::mapping::{DynMapping, MappingBinding};

pub trait AsKernelContext {
    fn as_kernel_context(&self) -> Arc<KernelContext>;
    fn at<I: FieldIndex>(&self, index: I) -> Element<I> {
        Element::new(index, Context::new(self.as_kernel_context()))
    }
}
impl AsKernelContext for Arc<KernelContext> {
    fn as_kernel_context(&self) -> Arc<KernelContext> {
        self.clone()
    }
}
impl<I: FieldIndex> AsKernelContext for Element<I> {
    fn as_kernel_context(&self) -> Arc<KernelContext> {
        self.context().kernel.clone()
    }
}
impl AsKernelContext for Context {
    fn as_kernel_context(&self) -> Arc<KernelContext> {
        self.kernel.clone()
    }
}

pub struct Element<I: FieldIndex> {
    index: I,
    context: Rc<RefCell<Context>>,
    active_context_index: Index,
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
            active_context_index,
        }
    }
    pub fn index(&self) -> &I {
        &self.index
    }
    pub fn context(&self) -> RefMut<'_, Context> {
        self.context.borrow_mut()
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
impl<I: FieldIndex> Drop for Element<I> {
    fn drop(&mut self) {
        ACTIVE_CONTEXTS.with(|active_contexts| {
            active_contexts
                .borrow_mut()
                .contexts
                .remove(self.active_context_index);
        });
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

pub struct FieldCache {
    cache_stack: Vec<HashMap<FieldId, Box<dyn Any>>>,
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
    pub kernel: Arc<KernelContext>,
    pub(crate) context_stack: Vec<HashMap<FieldId, HashSet<AccessLevel>>>,
}
impl Context {
    pub fn new(kernel: Arc<KernelContext>) -> Self {
        Self {
            bindings: HashMap::new(),
            cache: FieldCache::new(),
            kernel,
            context_stack: vec![HashMap::new()],
        }
    }
    pub fn get_cache<X: 'static>(&mut self, field: FieldId) -> Option<&mut X> {
        for cache in self.cache.cache_stack.iter_mut().rev() {
            if let Some(value) = cache.get_mut(&field) {
                return Some(value.downcast_mut().unwrap());
            }
        }
        None
    }
    pub fn insert_cache<X: 'static>(&mut self, field: FieldId, value: X) {
        self.cache
            .cache_stack
            .last_mut()
            .unwrap()
            .insert(field, Box::new(value));
    }
    // TODO: Why can't this work the normal way?
    pub fn get_cache_or_insert_with<X: 'static, R>(
        &mut self,
        field: FieldId,
        f: impl FnOnce(&mut Self) -> X,
        ret: impl FnOnce(&mut X) -> R,
    ) -> R {
        if let Some(value) = self.get_cache(field) {
            return ret(value);
        }
        let value = f(self);
        self.insert_cache(field, value);
        ret(self
            .cache
            .cache_stack
            .last_mut()
            .unwrap()
            .get_mut(&field)
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
                    mapping.save_dyn(i, ctx, field);
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
