use std::any::Any;
use std::marker::PhantomData;

use luisa::lang::types::AtomicRef;

use crate::field::access::{AccessCons, AccessLevel, AccessList, AccessNil, ListAccess};
use crate::field::Static;
use crate::internal_prelude::*;

pub mod bindless;
pub mod buffer;
pub mod cache;
pub mod constant;
pub mod function;
pub mod index;
// pub mod swap;

pub trait ListMapping<L: AccessList, I: 'static> {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldBinding,
    ) -> Box<dyn Any>;
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldBinding);
}

impl<I: 'static, T> ListMapping<AccessNil, I> for T {
    fn access_dyn(
        &self,
        _level: AccessLevel,
        _index: &dyn Any,
        _ctx: &mut Context,
        _binding: FieldBinding,
    ) -> Box<dyn Any> {
        unreachable!("Paradox");
    }
    fn save_dyn(&self, _level: AccessLevel, _ctx: &mut Context, _binding: FieldBinding) {}
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
        binding: FieldBinding,
    ) -> Box<dyn Any> {
        if level == X::level() {
            let index = index.downcast_ref().unwrap();
            let value = self.access(index, ctx, binding);
            Box::new(value)
        } else {
            <T as ListMapping<L, I>>::access_dyn(self, level, index, ctx, binding)
        }
    }
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldBinding) {
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
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldBinding) -> X;
    /// Saves the value of the field to the context. After this, the cached value should be droppable.
    #[allow(unused_variables)]
    fn save(&self, ctx: &mut Context, binding: FieldBinding) {}
}
pub trait EEMapping<V: Value, I: Value>: Mapping<Expr<V>, Expr<I>> {}
pub trait VEMapping<V: Value, I: Value>: Mapping<Var<V>, Expr<I>> {}
pub trait AEMapping<V: Value, I: Value>: Mapping<AtomicRef<V>, Expr<I>> {}
pub trait SEMapping<V: Value, I: Value>: Mapping<Static<V>, Expr<I>> {}
pub trait EMapping<V: Value, I: 'static>: Mapping<Expr<V>, I> {}
pub trait VMapping<V: Value, I: 'static>: Mapping<Var<V>, I> {}
pub trait AMapping<V: Value, I: 'static>: Mapping<AtomicRef<V>, I> {}
pub trait SMapping<V: Value, I: 'static>: Mapping<Static<V>, I> {}
impl<V: Value, I: Value, X> EEMapping<V, I> for X where X: Mapping<Expr<V>, Expr<I>> {}
impl<V: Value, I: Value, X> VEMapping<V, I> for X where X: Mapping<Var<V>, Expr<I>> {}
impl<V: Value, I: Value, X> AEMapping<V, I> for X where X: Mapping<AtomicRef<V>, Expr<I>> {}
impl<V: Value, I: Value, X> SEMapping<V, I> for X where X: Mapping<Static<V>, Expr<I>> {}
impl<V: Value, I: 'static, X> EMapping<V, I> for X where X: Mapping<Expr<V>, I> {}
impl<V: Value, I: 'static, X> VMapping<V, I> for X where X: Mapping<Var<V>, I> {}
impl<V: Value, I: 'static, X> AMapping<V, I> for X where X: Mapping<AtomicRef<V>, I> {}
impl<V: Value, I: 'static, X> SMapping<V, I> for X where X: Mapping<Static<V>, I> {}

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
        binding: FieldBinding,
    ) -> Box<dyn Any>;
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldBinding);
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
        binding: FieldBinding,
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
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldBinding) {
        debug_assert!(level <= X::level());
        <M as ListMapping<<X as ListAccess>::List, I>>::save_dyn(
            &self.mapping,
            level,
            ctx,
            binding,
        );
    }
}
