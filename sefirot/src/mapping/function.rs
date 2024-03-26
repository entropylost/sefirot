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
    fn access(&self, index: &I, ctx: &mut Context, _binding: FieldBinding) -> X {
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
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldBinding) -> X {
        ctx.get_cache_or_insert_with::<X, _>(&binding, |ctx| (self.f)(index, ctx), |v| v.clone())
    }
}

pub struct FieldMapping<
    X: Access,
    Y: Access<List = AccessCons<Y, AccessNil>>,
    I: FieldIndex,
    F: Fn(X) -> Y + 'static,
> {
    pub(crate) field: Field<X, I>,
    pub(crate) f: F,
    pub(crate) _marker: PhantomData<fn(X) -> Y>,
}
impl<
        X: Access,
        Y: Access<List = AccessCons<Y, AccessNil>>,
        I: FieldIndex,
        F: Fn(X) -> Y + 'static,
    > Mapping<Y, I> for FieldMapping<X, Y, I, F>
{
    fn access(&self, index: &I, ctx: &mut Context, _binding: FieldBinding) -> Y {
        let value = self.field.at_split(index, ctx);
        (self.f)(value)
    }
}
