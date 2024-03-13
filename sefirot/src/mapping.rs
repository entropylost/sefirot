use std::any::Any;
use std::marker::PhantomData;

use crate::field::AccessLevel;
use crate::internal_prelude::*;

pub trait Mapping<X: Access, I>:
    MappingLoopback<X, I, Chain = Self> + Send + Sync + 'static
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> X;
}

impl<X, I> Mapping<Paradox, I> for X
where
    X: Send + Sync + 'static,
{
    fn access(&self, _index: &I, _ctx: &mut Context, _binding: FieldHandle) -> Paradox {
        unreachable!("Paradox")
    }
}

mod loopback {

    use super::*;
    use crate::field::AccessLevel;
    pub trait MappingLoopback<X: Access, I> {
        type Chain: Mapping<X::Downcast, I>;
        fn access_loopback(
            &self,
            level: AccessLevel,
            index: &I,
            ctx: &mut Context,
            binding: FieldHandle,
        ) -> Box<dyn Any>;
    }
    impl<M, X: Access, I> MappingLoopback<X, I> for M
    where
        M: Mapping<X::Downcast, I>,
    {
        type Chain = M;
        fn access_loopback(
            &self,
            level: AccessLevel,
            index: &I,
            ctx: &mut Context,
            binding: FieldHandle,
        ) -> Box<dyn Any> {
            access_dyn::<X::Downcast, I, M>(self, level, index, ctx, binding)
        }
    }

    pub(crate) fn access_dyn<X: Access, I, M: Mapping<X, I>>(
        mapping: &M,
        level: AccessLevel,
        index: &I,
        ctx: &mut Context,
        binding: FieldHandle,
    ) -> Box<dyn Any> {
        if level == X::level() {
            let value = mapping.access(index, ctx, binding);
            Box::new(value)
        } else {
            M::access_loopback(mapping, AccessLevel(level.0 - 1), index, ctx, binding)
        }
    }
}
use loopback::MappingLoopback;

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
impl<X: Access, T: EmanationType, M: Mapping<X, T::Index>> DynMapping for MappingBinding<X, T, M> {
    fn access_dyn(
        &self,
        level: AccessLevel,
        index: &dyn Any,
        ctx: &mut Context,
        binding: FieldHandle,
    ) -> Box<dyn Any> {
        debug_assert!(level <= X::level());
        loopback::access_dyn::<X, T::Index, M>(
            &self.mapping,
            level,
            index.downcast_ref().unwrap(),
            ctx,
            binding,
        )
    }
}
