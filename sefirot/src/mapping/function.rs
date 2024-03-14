use std::marker::PhantomData;

use crate::field::access::{AccessCons, AccessNil};
use crate::internal_prelude::*;

pub struct FnMapping<
    X: Access<List = AccessCons<X, AccessNil>>,
    I: 'static,
    F: Fn(&I, &mut Context) -> X + Send + Sync + 'static,
> {
    f: F,
    _marker: PhantomData<fn(I) -> X>,
}
impl<
        X: Access<List = AccessCons<X, AccessNil>>,
        I: 'static,
        F: Fn(&I, &mut Context) -> X + Send + Sync + 'static,
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
        F: Fn(&I, &mut Context) -> X + Send + Sync + 'static,
    > Mapping<X, I> for FnMapping<X, I, F>
where
    F: Fn(&I, &mut Context) -> X,
{
    fn access(&self, index: &I, ctx: &mut Context, _binding: FieldHandle) -> X {
        (self.f)(index, ctx)
    }
}

pub struct CachedFnMapping<
    X: Access<List = AccessCons<X, AccessNil>> + Clone,
    I: 'static,
    F: Fn(&I, &mut Context) -> X + Send + Sync + 'static,
> {
    f: F,
    _marker: PhantomData<fn(I) -> X>,
}
impl<
        X: Access<List = AccessCons<X, AccessNil>> + Clone,
        I: 'static,
        F: Fn(&I, &mut Context) -> X + Send + Sync + 'static,
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
        F: Fn(&I, &mut Context) -> X + Send + Sync + 'static,
    > Mapping<X, I> for CachedFnMapping<X, I, F>
where
    F: Fn(&I, &mut Context) -> X,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> X {
        #[allow(clippy::map_entry)]
        if !ctx.cache.contains_key(&binding) {
            let value = (self.f)(index, ctx);
            ctx.cache.insert(binding, Box::new(value));
        }
        let value = ctx.cache.get(&binding).unwrap();
        value.downcast_ref::<X>().unwrap().clone()
    }
}
