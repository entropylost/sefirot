use luisa::lang::types::vector::Vec3;
use sefirot::graph::ComputeGraph;
use sefirot::prelude::*;
use sefirot::track_nc;

fn main() {
    let t1 = DEVICE.create_tex2d::<Vec3<f32>>(PixelStorage::Float4, 1920 * 2, 1080 * 2, 1);
    let t2 = DEVICE.create_tex2d::<Vec3<f32>>(PixelStorage::Float4, 1920 * 2, 1080 * 2, 1);
    let kernel = DEVICE.create_kernel::<fn()>(&track_nc!(|| {
        t2.write(dispatch_id().xy(), t1.read(dispatch_id().xy()));
    }));
    let mut total_time: f32 = 0.0;
    for _ in 0..200 {
        let mut graph = ComputeGraph::new();
        graph.add(kernel.dispatch_async([1920 * 2, 1080 * 2, 1]).debug("copy"));
        total_time += graph.execute_timed()[0].1;
    }
    println!("Average time: {}", total_time / 200.0);
}
