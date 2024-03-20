use std::marker::PhantomData;

use crate::field::access::{AccessCons, AccessNil};
use crate::internal_prelude::*;

pub struct FnMapping<
    X: Access<List = AccessCons<X, AccessNil>>,
    I: 'static,
    F: Fn(&I, &mut Context) -> X + 'static,
> {
    f: F,
    _marker: PhantomData<fn(I) -> X>,
}
impl<
        X: Access<List = AccessCons<X, AccessNil>>,
        I: 'static,
        F: Fn(&I, &mut Context) -> X + 'static,
    > FnMapping<X, I, F>
{
    pub fn new(f: F) -> Self {
        Self {
            f,
            _marker: PhantomData,
        }
    }
}
impl<
        X: Access<List = AccessCons<X, AccessNil>>,
        I: 'static,
        F: Fn(&I, &mut Context) -> X + 'static,
    > Mapping<X, I> for FnMapping<X, I, F>
where
    F: Fn(&I, &mut Context) -> X,
{
    fn access(&self, index: &I, ctx: &mut Context, _binding: FieldId) -> X {
        (self.f)(index, ctx)
    }
}

pub struct CachedFnMapping<
    X: Access<List = AccessCons<X, AccessNil>> + Clone,
    I: 'static,
    F: Fn(&I, &mut Context) -> X + 'static,
> {
    f: F,
    _marker: PhantomData<fn(I) -> X>,
}
impl<
        X: Access<List = AccessCons<X, AccessNil>> + Clone,
        I: 'static,
        F: Fn(&I, &mut Context) -> X + 'static,
    > CachedFnMapping<X, I, F>
{
    pub fn new(f: F) -> Self {
        Self {
            f,
            _marker: PhantomData,
        }
    }
}
impl<
        X: Access<List = AccessCons<X, AccessNil>> + Clone,
        I: 'static,
        F: Fn(&I, &mut Context) -> X + 'static,
    > Mapping<X, I> for CachedFnMapping<X, I, F>
where
    F: Fn(&I, &mut Context) -> X,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldId) -> X {
        ctx.get_cache_or_insert_with::<X, _>(binding, |ctx| (self.f)(index, ctx), |v| v.clone())
    }
}
