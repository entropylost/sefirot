//! This module exposes helper functions and traits for implementing a simple [`Mapping`] using a cached value,
//! using getters and setters. In order to use this, implement [`SimpleExprMapping`] for a given type,
//! and then use the `impl_cache_mapping!` macro.

pub use crate::impl_cache_mapping;
use crate::internal_prelude::*;

/// The cache type used in [`get_cache`] and [`save_cache`]
pub struct VarCache<V: Value, I> {
    pub value: Var<V>,
    pub index: I,
}

pub fn get_cache<'a, V: Value, I: FieldIndex, M: SimpleExprMapping<V, I>>(
    this: &'a M,
    index: &I,
    ctx: &'a mut Context,
    binding: FieldId,
) -> &'a mut VarCache<V, I> {
    #[allow(clippy::map_entry)]
    if !ctx.cache.contains_key(&binding) {
        let value = this.get_expr(index, ctx, binding).var();
        ctx.cache.insert(
            binding,
            Box::new(VarCache {
                value,
                index: index.clone(),
            }),
        );
    }
    ctx.cache.get_mut(&binding).unwrap().downcast_mut().unwrap()
}
pub fn save_cache<'a, V: Value, I: FieldIndex, M: SimpleExprMapping<V, I>>(
    this: &'a M,
    ctx: &'a mut Context,
    binding: FieldId,
) {
    let cache = ctx
        .cache
        .remove(&binding)
        .unwrap()
        .downcast::<VarCache<V, I>>()
        .unwrap();
    this.set_expr(&cache.index, **cache.value, ctx, binding);
}

/// A trait describing a way of getting and setting a value given an index.
/// See [`CachedMapping`] for use.
pub trait SimpleExprMapping<V: Value, I: FieldIndex>: Send + Sync + 'static {
    fn get_expr(&self, index: &I, ctx: &mut Context, binding: FieldId) -> Expr<V>;
    fn set_expr(&self, index: &I, value: Expr<V>, ctx: &mut Context, binding: FieldId);
}

#[macro_export]
macro_rules! impl_cache_mapping {
    ($([ $($t:tt)* ])? Mapping[$V:ty, $I:ty] for $X:ty $(where $($where_clause:tt)*)?) => {
        impl $(<$($t)*>)? $crate::mapping::Mapping<Expr<$V>, $I> for $X $(where $($where_clause)*)? {
            fn access(&self, index: &$I, ctx: &mut $crate::element::Context, binding: $crate::field::FieldId) -> Expr<$V> {
                **$crate::mapping::cache::get_cache(self, index, ctx, binding).value
            }
        }
        impl $(<$($t)*>)? $crate::mapping::Mapping<Var<$V>, $I> for $X $(where $($where_clause)*)? {
            fn access(&self, index: &$I, ctx: &mut $crate::element::Context, binding: $crate::field::FieldId) -> Var<$V> {
                $crate::mapping::cache::get_cache(self, index, ctx, binding).value
            }
            fn save(&self, ctx: &mut $crate::element::Context, binding: $crate::field::FieldId) {
                $crate::mapping::cache::save_cache(self, ctx, binding);
            }
        }
    };
}
