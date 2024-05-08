//! This module exposes helper functions and traits for implementing a simple [`Mapping`] using a cached value,
//! using getters and setters. In order to use this, implement [`SimpleExprMapping`] for a given type,
//! and then use the `impl_cache_mapping!` macro.

pub use crate::impl_cache_mapping;
use crate::internal_prelude::*;

/// The cache type used in [`get_cache`] and [`save_cache`]
#[derive(Clone)]
pub struct VarCache<V: Value, I> {
    pub value: Var<V>,
    pub index: I,
}

pub fn get_value<'a, V: Value, I: FieldIndex, M: SimpleExprMapping<V, I>>(
    this: &'a M,
    index: &I,
    ctx: &'a mut Context,
    binding: FieldBinding,
) -> Var<V> {
    ctx.get_cache_or_insert_with::<VarCache<V, I>, _>(
        &binding,
        |ctx| {
            let value = this.get_expr(index, ctx).var();
            VarCache {
                value,
                index: index.clone(),
            }
        },
        |cache| cache.value,
    )
}
pub fn save_cache<'a, V: Value, I: FieldIndex, M: SimpleExprMapping<V, I>>(
    this: &'a M,
    ctx: &'a mut Context,
    binding: FieldBinding,
) {
    if let Some(cache) = ctx.get_cache::<VarCache<V, I>>(&binding).cloned() {
        this.set_expr(&cache.index, **cache.value, ctx);
    }
}

/// A trait describing a way of getting and setting a value given an index.
/// See [`CachedMapping`] for use.
pub trait SimpleExprMapping<V: Value, I: FieldIndex>: Send + Sync + 'static {
    // TODO: type Ext: ExtensionList<Self>;
    fn get_expr(&self, index: &I, ctx: &mut Context) -> Expr<V>;
    fn set_expr(&self, index: &I, value: Expr<V>, ctx: &mut Context);
}

#[macro_export]
macro_rules! impl_cache_mapping {
    ($([ $($t:tt)* ])? Mapping[$V:ty, $I:ty] for $X:ty $(where $($where_clause:tt)*)?) => {
        impl $(<$($t)*>)? $crate::mapping::Mapping<Expr<$V>, $I> for $X $(where $($where_clause)*)? {
            type Ext = ();
            fn access(&self, index: &$I, ctx: &mut $crate::element::Context, binding: $crate::element::FieldBinding) -> Expr<$V> {
                **$crate::mapping::cache::get_value(self, index, ctx, binding)
            }
        }
        impl $(<$($t)*>)? $crate::mapping::Mapping<Var<$V>, $I> for $X $(where $($where_clause)*)? {
            type Ext = ();
            fn access(&self, index: &$I, ctx: &mut $crate::element::Context, binding: $crate::element::FieldBinding) -> Var<$V> {
                $crate::mapping::cache::get_value(self, index, ctx, binding)
            }
            fn save(&self, ctx: &mut $crate::element::Context, binding: $crate::element::FieldBinding) {
                $crate::mapping::cache::save_cache(self, ctx, binding);
            }
        }
    };
}
