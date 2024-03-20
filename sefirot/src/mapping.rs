use std::any::Any;
use std::marker::PhantomData;

use crate::field::access::{AccessCons, AccessLevel, AccessList, AccessNil, ListAccess};
use crate::internal_prelude::*;

pub mod buffer;
pub mod cache;
pub mod function;
pub mod index;

mod list {
    use super::*;

    pub trait ListMapping<L: AccessList, I: 'static> {
        fn access_dyn(
            &self,
            level: AccessLevel,
            index: &dyn Any,
            ctx: &mut Context,
            binding: FieldId,
        ) -> Box<dyn Any>;
        fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldId);
    }
}
use list::ListMapping;
use luisa::lang::types::AtomicRef;
impl<I: 'static, T> ListMapping<AccessNil, I> for T {
    fn access_dyn(
        &self,
        _level: AccessLevel,
        _index: &dyn Any,
        _ctx: &mut Context,
        _binding: FieldId,
    ) -> Box<dyn Any> {
        unreachable!("Paradox");
    }
    fn save_dyn(&self, _level: AccessLevel, _ctx: &mut Context, _binding: FieldId) {}
}
impl<X: Access, L: AccessList, I: 'static, T> ListMapping<AccessCons<X, L>, I> for T
where
    T: Mapping<X, I> + ListMapping<L, I>,
{
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldId,
    ) -> Box<dyn Any> {
        if level == X::level() {
            let index = index.downcast_ref().unwrap();
            let value = self.access(index, ctx, binding);
            Box::new(value)
        } else {
            <T as ListMapping<L, I>>::access_dyn(self, level, index, ctx, binding)
        }
    }
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldId) {
        if level == X::level() {
            self.save(ctx, binding);
        } else {
            <T as ListMapping<L, I>>::save_dyn(self, level, ctx, binding);
        }
    }
}

pub trait Mapping<X: Access, I: 'static>:
    ListMapping<<X as ListAccess>::List, I> + 'static
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldId) -> X;
    /// Saves the value of the field to the context. After this, the cached value should be droppable.
    fn save(&self, _ctx: &mut Context, _binding: FieldId) {}
}
pub trait EMapping<V: Value, I: Value>: Mapping<Expr<V>, Expr<I>> {}
pub trait VMapping<V: Value, I: Value>: Mapping<Var<V>, Expr<I>> {}
pub trait AMapping<V: Value, I: Value>: Mapping<AtomicRef<V>, Expr<I>> {}
impl<V: Value, I: Value, X> EMapping<V, I> for X where X: Mapping<Expr<V>, Expr<I>> {}
impl<V: Value, I: Value, X> VMapping<V, I> for X where X: Mapping<Var<V>, Expr<I>> {}
impl<V: Value, I: Value, X> AMapping<V, I> for X where X: Mapping<AtomicRef<V>, Expr<I>> {}

mod private {
    use super::*;
    pub trait Sealed {}
    impl<X: Access, I: FieldIndex, M: Mapping<X, I>> Sealed for MappingBinding<X, I, M> {}
}

pub trait DynMapping: 'static + private::Sealed {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldId,
    ) -> Box<dyn Any>;
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldId);
}

pub(crate) struct MappingBinding<X: Access, I: FieldIndex, M: Mapping<X, I>> {
    pub(crate) mapping: M,
    pub(crate) _marker: PhantomData<fn() -> (X, I)>,
}
impl<X: Access, I: FieldIndex, M: Mapping<X, I>> MappingBinding<X, I, M> {
    pub(crate) fn new(mapping: M) -> Self {
        Self {
            mapping,
            _marker: PhantomData,
        }
    }
}
impl<X: Access, I: FieldIndex, M: Mapping<X, I>> DynMapping for MappingBinding<X, I, M> {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldId,
    ) -> Box<dyn Any> {
        debug_assert!(level <= X::level());
        <M as ListMapping<<X as ListAccess>::List, I>>::access_dyn(
            &self.mapping,
            level,
            index,
            ctx,
            binding,
        )
    }
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldId) {
        debug_assert!(level <= X::level());
        <M as ListMapping<<X as ListAccess>::List, I>>::save_dyn(
            &self.mapping,
            level,
            ctx,
            binding,
        );
    }
}
