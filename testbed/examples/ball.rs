use keter::lang::types::vector::{Vec2, Vec3};
use keter::prelude::*;
use keter_testbed::App;
use yesod::camera::{Camera, View};
use yesod::direction::*;
use yesod::shapes::intersect_sphere;

fn main() {
    let app = App::new("Gradient", [1024; 2]).scale(2).resize().finish();

    let draw_kernel = DEVICE.create_kernel::<fn(View)>(&track!(|view| {
        set_block_size([8, 8, 1]);

        let pixel = dispatch_id().xy();
        let ray_dir = view.ray_dir(pixel.cast_f32() + 0.5).normalize();

        let (times, intersect) = intersect_sphere(view.pos, ray_dir, 1.0_f32.expr());
        if !intersect {
            return;
        }
        let dir = view.pos + ray_dir * times.x;
        let dir = dir.normalize();

        let encoded = ClarbergEncoder::encode(dir);

        let quantization_size = Vec2::new(8_u32, 4) * 2;
        let quantization_size = quantization_size.expr().cast_f32();
        let encoded = (encoded * quantization_size).floor() / (quantization_size - 1.0);

        let color = encoded.extend(0.0);
        app.set_pixel(pixel.cast_i32(), color);
    }));

    app.run(|rt| {
        let camera = Camera {
            screen_size: rt.size().map(|x| x as f32).into(),
            yaw: rt.tick as f32 * 0.003,
            pitch: -2.0 * (rt.active_cursor_position().y / rt.size()[1] as f32 - 0.5),
            pos: Vec3::splat(0.0).into(),
            fov: 1.15,
        }
        .orbit(5.0);

        draw_kernel.dispatch(rt.dispatch_size(), &camera.view());
    });
}
