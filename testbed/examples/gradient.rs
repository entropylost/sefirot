use luisa_compute::lang::types::vector::Vec3;
use sefirot::prelude::*;
use sefirot_testbed::init;

fn main() {
    let app = init("Gradient", [1024; 2], 2);
    let gradient_kernel = DEVICE.create_kernel::<fn()>(&track!(|| {
        let value = dispatch_id().x.cast_f32() / 1024.0;
        app.display().write(
            dispatch_id().xy(),
            Vec3::expr(0.5 - (value * 2.0).cos() / 2.0, 0.0, value.sin()),
        );
    }));
    app.run(|_rt, scope| {
        scope.submit([gradient_kernel.dispatch_async([1024, 1024, 1])]);
    });
}
