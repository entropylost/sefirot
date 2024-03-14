use crate::internal_prelude::*;

/// The cache element used in [`CachedMapping`] to track the variable.
pub struct VarCache<V: Value> {
    pub value: Var<V>,
    pub changed: bool,
}

pub struct CachedMapping<M>(pub M);

/// A helper trait to implement [`Mapping`] using getters and setters and a cache for the variable.
pub trait CachedMappingT<V: Value, I: 'static>: Send + Sync + 'static {
    fn get_expr(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Expr<V>;
    fn set_expr(&self, index: &I, value: Expr<V>, ctx: &mut Context, binding: FieldHandle);
    fn get_cache<'a>(
        &'a self,
        index: &I,
        ctx: &'a mut Context,
        binding: FieldHandle,
    ) -> &'a mut VarCache<V> {
        #[allow(clippy::map_entry)]
        if !ctx.cache.contains_key(&binding) {
            let value = self.get_expr(index, ctx, binding).var();
            ctx.cache.insert(
                binding,
                Box::new(VarCache {
                    value,
                    changed: false,
                }),
            );
        }
        ctx.cache.get_mut(&binding).unwrap().downcast_mut().unwrap()
    }
}
impl<X, V: Value, I: 'static> Mapping<Expr<V>, I> for CachedMapping<X>
where
    X: CachedMappingT<V, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Expr<V> {
        self.0.get_expr(index, ctx, binding)
    }
}
impl<X, V: Value, I: 'static> Mapping<Var<V>, I> for CachedMapping<X>
where
    X: CachedMappingT<V, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Var<V> {
        self.0.get_expr(index, ctx, binding).var()
    }
}
