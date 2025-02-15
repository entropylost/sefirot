use keter::lang::types::vector::Vec3;
use keter::prelude::*;
use keter_testbed::App;

fn main() {
    let app = App::new("Gradient", [1024; 2]).scale(2).resize().finish();
    let gradient_kernel = DEVICE.create_kernel::<fn()>(&track!(|| {
        let value = dispatch_id().x.cast_f32() / 1024.0;
        app.display().write(
            dispatch_id().xy(),
            Vec3::expr(0.5 - (value * 2.0).cos() / 2.0, 0.0, value.sin()),
        );
    }));
    app.run(|rt, scope| {
        scope.submit([gradient_kernel.dispatch_async(rt.dispatch_size())]);
    });
}
