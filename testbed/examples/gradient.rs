use luisa_compute::lang::types::vector::Vec3;
use sefirot::prelude::*;
use sefirot_testbed::init;

fn main() {
    let (app, device) = init("Gradient", [1024; 2], 2);
    let gradient_kernel = device.create_kernel::<fn()>(&track!(|| {
        let value = dispatch_id().x.cast_f32() / 1024.0;
        app.display()
            .write(dispatch_id().xy(), Vec3::splat_expr(value));
    }));
    app.run(|_rt, scope| {
        scope.submit([gradient_kernel.dispatch_async([1024, 1024, 1])]);
    });
}
