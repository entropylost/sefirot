use luisa::lang::types::AtomicRef;

use crate::field::Static;
use crate::internal_prelude::*;

/// The cache element used in [`CachedMapping`] to track the variable.
pub struct VarCache<V: Value, I> {
    pub value: Var<V>,
    pub index: I,
}

/// A helper to implement a [`Mapping`] using getters and setters and a cache for the variable, using the [`SimpleExprMapping`] trait.
/// This calls the [`SimpleExprMapping::set_expr`] once when the [`Context`] is dropped.
///
/// Note that this forwards [`AtomicRef`] and [`Static`] to the inner for conveinence,
/// but implementing non built-in [`Access`] types will require wrapping.
#[derive(Debug, Clone, Copy)]
pub struct VarCacheMapping<M>(pub M);

fn get_cache<'a, V: Value, I: FieldIndex, M: SimpleExprMapping<V, I>>(
    this: &'a M,
    index: &I,
    ctx: &'a mut Context,
    binding: FieldHandle,
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

/// A trait describing a way of getting and setting a value given an index.
/// See [`CachedMapping`] for use.
pub trait SimpleExprMapping<V: Value, I: FieldIndex>: Send + Sync + 'static {
    fn get_expr(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Expr<V>;
    fn set_expr(&self, index: &I, value: Expr<V>, ctx: &mut Context, binding: FieldHandle);
}
impl<X, V: Value, I: FieldIndex> Mapping<Expr<V>, I> for VarCacheMapping<X>
where
    X: SimpleExprMapping<V, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Expr<V> {
        **get_cache(&self.0, index, ctx, binding).value
    }
}
impl<X, V: Value, I: FieldIndex> Mapping<Var<V>, I> for VarCacheMapping<X>
where
    X: SimpleExprMapping<V, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Var<V> {
        get_cache(&self.0, index, ctx, binding).value
    }
    fn save(&self, ctx: &mut Context, binding: FieldHandle) {
        let cache = ctx
            .cache
            .remove(&binding)
            .unwrap()
            .downcast::<VarCache<V, I>>()
            .unwrap();
        self.0.set_expr(&cache.index, **cache.value, ctx, binding);
    }
}
impl<X, V: Value, I: FieldIndex> Mapping<AtomicRef<V>, I> for VarCacheMapping<X>
where
    X: SimpleExprMapping<V, I> + Mapping<AtomicRef<V>, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> AtomicRef<V> {
        self.0.access(index, ctx, binding)
    }
    fn save(&self, ctx: &mut Context, binding: FieldHandle) {
        self.0.save(ctx, binding);
    }
}
impl<X, V: Value, I: FieldIndex> Mapping<Static<V>, I> for VarCacheMapping<X>
where
    X: SimpleExprMapping<V, I> + Mapping<Static<V>, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Static<V> {
        self.0.access(index, ctx, binding)
    }
    fn save(&self, ctx: &mut Context, binding: FieldHandle) {
        self.0.save(ctx, binding);
    }
}
