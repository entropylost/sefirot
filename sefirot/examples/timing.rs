use std::env::current_exe;

use luisa::DeviceType;
use sefirot::graph::ComputeGraph;
use sefirot::prelude::*;
use sefirot::track_nc;

fn main() {
    let context = Context::new(current_exe().unwrap());
    let device = context.create_device(DeviceType::Cuda);
    let buffer = device.create_buffer::<bool>(1000000);
    let kernel = device.create_kernel::<fn()>(&track_nc!(|| {
        let i = dispatch_id().x;
        let is_prime = (i > 1).var();
        for j in 2.expr()..i {
            if i % j == 0 {
                *is_prime = false;
                break;
            }
        }
        buffer.write(i, is_prime);
    }));
    let mut graph = ComputeGraph::new(&device);
    graph.add(
        kernel
            .dispatch_async([buffer.len() as u32, 1, 1])
            .debug("primes"),
    );
    let start = std::time::Instant::now();
    graph.execute_trace();
    let elapsed = start.elapsed();
    println!("Elapsed: {:?}", elapsed);
}
