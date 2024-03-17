use std::env::current_exe;
use std::sync::Arc;

use parking_lot::Mutex;
use sefirot_macro::track_nc;

use crate::mapping::buffer::StaticDomain;
use crate::mapping::function::CachedFnMapping;
use crate::prelude::*;

#[test]
fn test_context_stack_if() {
    luisa::init_logger();
    let ctx = Context::new(current_exe().unwrap());
    let device = ctx.create_device("cuda");

    let mut fields = FieldSet::new();

    let num_accesses = Arc::new(Mutex::new(0));

    let na2 = num_accesses.clone();

    let domain = StaticDomain::<1>::new(16);
    let half_field: EField<u32, u32> = fields.create_bind(
        "half",
        CachedFnMapping::new(track_nc!(move |el, _ctx| {
            *na2.lock() += 1;
            *el / 2
        })),
    );

    let _kernel = Kernel::<fn()>::build(
        &device,
        &domain,
        track!(&|el| {
            let x = 0_u32.var();
            let cond = *el < 8;
            if cond {
                *x = half_field.expr(&el);
            }
            *x = half_field.expr(&el);
        }),
    );

    assert_eq!(*num_accesses.lock(), 2);

    let _kernel = Kernel::<fn()>::build(
        &device,
        &domain,
        track!(&|el| {
            let x = 0_u32.var();
            let cond = *el < 8;
            *x = half_field.expr(&el);
            if cond {
                *x = half_field.expr(&el);
            }
        }),
    );

    assert_eq!(*num_accesses.lock(), 3);
}
