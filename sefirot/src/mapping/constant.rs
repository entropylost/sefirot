use std::sync::Arc;

use parking_lot::Mutex;

use crate::internal_prelude::*;

#[derive(Debug, Clone)]
pub struct ConstantMapping<V: Value + Send> {
    pub current_value: Arc<Mutex<V>>,
}

impl<V: Value + Send, I: FieldIndex> Mapping<Expr<V>, I> for ConstantMapping<V> {
    type Ext = ();
    fn access(&self, _index: &I, ctx: &mut Context, binding: FieldBinding) -> Expr<V> {
        ctx.get_cache_or_insert_with_global(
            &binding,
            |ctx| {
                let value = self.current_value.clone();
                ctx.bind_arg(move || *value.lock())
            },
            |x| *x,
        )
    }
}
impl ConstantMapping<f32> {
    pub fn new(value: f32) -> Self {
        Self {
            current_value: Arc::new(Mutex::new(value)),
        }
    }
}

#[test]
fn test_scope_loss() {
    use std::env::current_exe;

    use luisa::DeviceType;
    use sefirot_macro::track_nc;

    use super::buffer::StaticDomain;
    use crate::field::set::FieldSet;
    use crate::field::EEField;
    use crate::kernel::Kernel;
    let context = luisa::Context::new(current_exe().unwrap());
    let device = context.create_device(DeviceType::Cuda);
    let domain = StaticDomain::<1>::new(10);
    let mut fields = FieldSet::new();
    let constant: EEField<f32, u32> = fields.create_bind("constant", ConstantMapping::new(10.0));
    let _kernel = Kernel::<fn()>::build(
        &device,
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
