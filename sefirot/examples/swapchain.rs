use luisa_compute::DeviceType;
use sefirot::prelude::*;

fn main() {
    luisa::init_logger();
    let ctx = Context::new(std::env::current_exe().unwrap());
    let device = ctx.create_device(DeviceType::Cuda);
    use winit::event_loop::EventLoop;
    let event_loop = EventLoop::new().unwrap();
    let window = event_loop
        .create_window(
            winit::window::Window::default_attributes()
                .with_inner_size(winit::dpi::PhysicalSize::new(100, 100)),
        )
        .unwrap();

    let swapchain =
        device.create_swapchain(window, &device.default_stream(), 100, 100, false, false, 3);
    println!("After");
    loop {}
}
