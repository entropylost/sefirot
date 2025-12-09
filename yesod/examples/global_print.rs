use keter::lang::types::vector::Vec3;
use keter::prelude::*;
use yesod::device_println;
use yesod::printer::global::*;

fn main() {
    set_capacity(64);
    let test_fn = DEVICE.create_kernel::<fn(f32)>(&track!(|i| {
        let index = dispatch_id().x;
        let val = i * index.cast_f32();
        let v = Vec3::<f32>::expr(1.0, 5.0, 3.0);
        device_println!(
            "Value at index {}: {}\n{:?}",
            index.host(),
            val.host(),
            v.host(),
        );
    }));
    println!("Starting");
    test_fn.dispatch([4, 1, 1], &0.3);
    test_fn.dispatch([4, 1, 1], &0.7);
    DEVICE.default_stream().scope().synchronize();
    println!("Finished");
    flush_printer();
}
