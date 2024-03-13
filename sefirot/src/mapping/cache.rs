use crate::internal_prelude::*;

/// The cache element used in [`CachedMapping`] to track the variable.
pub struct VarCache<V: Value> {
    pub value: Var<V>,
    pub changed: bool,
}

/// A helper trait to implement [`Mapping`] using getters and setters and a cache for the variable.
pub trait CachedMapping<V: Value, I>: Send + Sync + 'static {
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
impl<X, V: Value, I> Mapping<Expr<V>, I> for X
where
    X: CachedMapping<V, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Expr<V> {
        self.get_expr(index, ctx, binding)
    }
}
impl<X, V: Value, I> Mapping<Var<V>, I> for X
where
    X: CachedMapping<V, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldHandle) -> Var<V> {
        self.get_expr(index, ctx, binding).var()
    }
}
