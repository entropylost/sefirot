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
            binding: FieldHandle,
        ) -> Box<dyn Any>;
        fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldHandle);
    }
}
use list::ListMapping;
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
    fn save_dyn(&self, _level: AccessLevel, _ctx: &mut Context, _binding: FieldHandle) {}
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
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldHandle) {
        if level == X::level() {
            self.save(ctx, binding);
        } else {
            <T as ListMapping<L, I>>::save_dyn(self, level, ctx, binding);
        }
    }
}

pub trait Mapping<X: Access, I: 'static>:
    ListMapping<<X as ListAccess>::List, I> + Send + Sync + 'static
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> X;
    fn save(&self, _ctx: &mut Context, _binding: FieldHandle) {}
}

mod private {
    use super::*;
    pub trait Sealed {}
    impl<X: Access, T: EmanationType, M: Mapping<X, T::Index>> Sealed for MappingBinding<X, T, M> {}
}

pub trait DynMapping: Send + Sync + 'static + private::Sealed {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldHandle,
    ) -> Box<dyn Any>;
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldHandle);
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
    fn save_dyn(&self, level: AccessLevel, ctx: &mut Context, binding: FieldHandle) {
        debug_assert!(level <= X::level());
        <M as ListMapping<<X as ListAccess>::List, T::Index>>::save_dyn(
            &self.mapping,
            level,
            ctx,
            binding,
        );
    }
}
