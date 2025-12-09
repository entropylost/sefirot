use std::sync::LazyLock;

use keter::lang::types::vector::{Vec2, Vec4};
use keter::prelude::*;

#[allow(clippy::type_complexity)]
static DRAW_LINE_KERNEL: LazyLock<Kernel<fn(Tex2d<Vec4<f32>>, Vec2<f32>, Vec2<f32>, Vec4<f32>)>> =
    LazyLock::new(|| {
        DEVICE.create_kernel::<fn(Tex2d<Vec4<f32>>, Vec2<f32>, Vec2<f32>, Vec4<f32>)>(&track!(
            |display, start, end, color| {
                let t = dispatch_id().x.cast_f32() / (dispatch_size().x - 1).cast_f32();
                let pos = start + (end - start) * t;
                display.write(pos.cast_i32().cast_u32(), color);
            }
        ))
    });
pub fn draw_line(display: &Tex2d<Vec4<f32>>, start: Vec2<f32>, end: Vec2<f32>, color: Vec4<f32>) {
    DRAW_LINE_KERNEL.dispatch(
        [display.width().max(display.height()), 1, 1],
        display,
        &start,
        &end,
        &color,
    );
}
