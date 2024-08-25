use std::sync::Arc;

use luisa_compute::runtime::KernelArg;
use parking_lot::RwLock;

use crate::field::Static;
use crate::internal_prelude::*;

#[derive(Debug, Clone)]
pub struct ConstantMapping<V: Value + Send + Sync> {
    pub value: Arc<RwLock<V>>,
}
// Perhaps make the index `()`? Although would need to have different values for different dispatches in like the dual
// double invocation.
impl<V: Value + Send + Sync, I: FieldIndex> Mapping<Expr<V>, I> for ConstantMapping<V> {
    type Ext = ();
    fn access(&self, _index: &I, ctx: &mut Context, binding: FieldBinding) -> Expr<V> {
        ctx.get_cache_or_insert_with_global(
            &binding,
            |ctx| {
                let value = self.value.clone();
                ctx.bind_arg(move || *value.read())
            },
            |x| *x,
        )
    }
}
impl<V: Value + Send + Sync> ConstantMapping<V> {
    pub fn new(value: Arc<RwLock<V>>) -> Self {
        Self { value }
    }
    pub fn with_value(value: V) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
        }
    }
}

// This wraps the result in `Static` to make it compatible with the `Mapping` trait.
#[derive(Debug, Clone)]
pub struct ArgumentMapping<A: KernelArg + Send + Sync + 'static> {
    pub value: Arc<RwLock<A>>,
}
// Perhaps make the index `()`? Although would need to have different values for different dispatches in like the dual
// double invocation.
impl<A: KernelArg + Send + Sync + 'static, I: FieldIndex> Mapping<Static<A::Parameter>, I>
    for ArgumentMapping<A>
where
    A::Parameter: Clone,
{
    type Ext = ();
    fn access(&self, _index: &I, ctx: &mut Context, binding: FieldBinding) -> Static<A::Parameter> {
        ctx.get_cache_or_insert_with_global(
            &binding,
            |ctx| {
                let value = self.value.clone();
                Static(ctx.bind_arg_indirect(move || value.read_arc()))
            },
            |x| x.clone(),
        )
    }
}
impl<A: KernelArg + Send + Sync + 'static> ArgumentMapping<A> {
    pub fn new(value: Arc<RwLock<A>>) -> Self {
        Self { value }
    }
    pub fn with_value(value: A) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
        }
    }
}

#[test]
fn test_scope_loss() {
    use sefirot_macro::track_nc;

    use super::buffer::StaticDomain;
    use crate::field::set::FieldSet;
    use crate::field::EEField;
    use crate::kernel::Kernel;
    let domain = StaticDomain::<1>::new(10);
    let mut fields = FieldSet::new();
    let constant: EEField<f32, u32> =
        fields.create_bind("constant", ConstantMapping::with_value(10.0));
    let _kernel = Kernel::<fn()>::build(
        &domain,
        track_nc!(&|el| {
            let cond = true.expr();
            if cond {
                constant.expr(&el);
            }
            let _a = constant.expr(&el) + 1.0;
        }),
    );
}
