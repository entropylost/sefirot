use std::sync::Arc;

use parking_lot::Mutex;
use sefirot_macro::track_nc;

use crate::mapping::buffer::StaticDomain;
use crate::mapping::function::CachedFnMapping;
use crate::prelude::*;

#[test]
fn test_context_stack_if() {
    let mut fields = FieldSet::new();

    let num_accesses = Arc::new(Mutex::new(0));

    let na2 = num_accesses.clone();

    let domain = StaticDomain::<1>::new(16);
    let half_field: EEField<u32, u32> = fields.create_bind(
        "half",
        CachedFnMapping::new(track_nc!(move |el, _ctx| {
            *na2.lock() += 1;
            *el / 2
        })),
    );

    let _kernel = Kernel::<fn()>::build(
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

#[test]
fn test_return() {
    let domain = StaticDomain::<1>::new(2);
    let mut fields = FieldSet::new();
    let buffer = DEVICE.create_buffer::<u32>(2);
    let field: AEField<u32, u32> = fields.create_bind("data", domain.map_buffer(&buffer));

    let kernel = Kernel::<fn()>::build(
        &domain,
        track!(&|el| {
            let _ = field.var(&el);
            if *el == 0 {
                *field.var(&el) = 1_u32;
                return;
            }
            *field.var(&el) = 2_u32;
        }),
    );
    kernel.dispatch_blocking();

    assert_eq!(buffer.copy_to_vec(), vec![1, 2]);
}
