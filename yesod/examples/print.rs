use keter::lang::types::vector::Vec3;
use keter::prelude::*;
use yesod::printer::*;

fn main() {
    let print_buffer = PrintBuffer::new(1024);
    let test_fn = DEVICE.create_kernel::<fn(f32)>(&track!(|i| {
        let index = dispatch_id().x;
        let val = i * index.cast_f32();
        let v = Vec3::<f32>::expr(1.0, 5.0, 3.0);
        print_buffer.print(move |printer| {
            format!(
                "Value at index {}: {}\n{:?}\n",
                printer.load(index),
                printer.load(val),
                printer.load(v),
            )
        });
    }));
    println!("Starting");
    test_fn.dispatch([4, 1, 1], &0.3);
    test_fn.dispatch([4, 1, 1], &0.7);
    DEVICE.default_stream().scope().synchronize();
    println!("Finished");
    print_buffer.flush();
}
