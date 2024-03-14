use std::any::Any;
use std::marker::PhantomData;

use crate::field::{AccessCons, AccessLevel, AccessList, AccessNil, ListAccess};
use crate::internal_prelude::*;

pub mod buffer;
pub mod cache;
pub mod index;

pub trait ListMapping<L: AccessList, I: 'static> {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldHandle,
    ) -> Box<dyn Any>;
}
impl<I: 'static, T> ListMapping<AccessNil, I> for T {
    fn access_dyn(
        &self,
        _level: AccessLevel,
        _index: &dyn Any,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) -> Box<dyn Any> {
        unreachable!("Paradox");
    }
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
        binding: FieldHandle,
    ) -> Box<dyn Any> {
        if level == X::level() {
            let index = index.downcast_ref().unwrap();
            let value = self.access(index, ctx, binding);
            Box::new(value)
        } else {
            <T as ListMapping<L, I>>::access_dyn(self, level, index, ctx, binding)
        }
    }
}

pub trait Mapping<X: Access, I: 'static>:
    ListMapping<<X as ListAccess>::List, I> + Send + Sync + 'static
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> X;
}

pub(crate) trait DynMapping: Send + Sync + 'static {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldHandle,
    ) -> Box<dyn Any>;
}

pub(crate) struct MappingBinding<X: Access, T: EmanationType, M: Mapping<X, T::Index>> {
    pub(crate) mapping: M,
    pub(crate) _marker: PhantomData<fn() -> (X, T)>,
}
impl<X: Access, T: EmanationType, M: Mapping<X, T::Index>> MappingBinding<X, T, M> {
    pub(crate) fn new(mapping: M) -> Self {
        Self {
            mapping,
            _marker: PhantomData,
        }
    }
}
impl<X: Access, T: EmanationType, M: Mapping<X, T::Index>> DynMapping for MappingBinding<X, T, M> {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldHandle,
    ) -> Box<dyn Any> {
        debug_assert!(level <= X::level());
        <M as ListMapping<<X as ListAccess>::List, T::Index>>::access_dyn(
            &self.mapping,
            level,
            index,
            ctx,
            binding,
        )
    }
}
