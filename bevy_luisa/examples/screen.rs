use bevy::prelude::*;
use bevy_luisa::display::{present_swapchain_and_clear, ClearColor, LuisaDisplayPlugin};
use bevy_luisa::luisa::lang::types::vector::Vec4;
use bevy_luisa::prelude::*;

fn main() {
    App::new()
        .init_resource::<ClearColor>()
        .add_plugins(DefaultPlugins)
        .add_plugins(LuisaPlugin::default())
        .add_plugins(LuisaDisplayPlugin::default())
        .add_systems(PostUpdate, present_swapchain_and_clear)
        .add_systems(Update, update_clear_color)
        .run();
}

fn update_clear_color(time: Res<Time>, mut clear_color: ResMut<ClearColor>) {
    let s = time.elapsed_seconds();
    *clear_color = ClearColor(Vec4::new(
        (s * 0.1).sin() * 0.5 + 0.5,
        (s * 0.2).sin() * 0.5 + 0.5,
        (s * 0.3).sin() * 0.5 + 0.5,
        1.0,
    ));
}
