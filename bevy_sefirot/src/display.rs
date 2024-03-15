use bevy::prelude::*;
use bevy::winit::WinitWindows;
use luisa::lang::types::vector::{Vec2, Vec4};
use sefirot::mapping::buffer::StaticDomain;

use super::prelude::*;

#[derive(Component)]
pub struct DisplayTexture {
    pub fields: FieldSet,
    /// The color texture for the display. Note that the format is not guaranteed,
    /// so this should not be used for intermediate calculations.
    pub color: VField<Vec4<f32>, Vec2<u32>>,
    /// The actual texture that will be displayed.
    pub color_texture: Tex2d<Vec4<f32>>,
    /// The domain of the screen.
    // TODO: Implement resizing.
    pub domain: StaticDomain<2>,
}

#[derive(Deref, Component)]
pub struct LuisaSwapchain(pub Swapchain);

#[derive(Deref, DerefMut, Resource, Copy, Clone, PartialEq, Debug)]
pub struct ClearColor(pub Vec4<f32>);
impl Default for ClearColor {
    fn default() -> Self {
        Self(Vec4::new(0.0, 0.0, 0.0, 1.0))
    }
}

// TODO: Perhaps add an update_dipslay somewhere?
pub fn setup_display(
    mut commands: Commands,
    device: Res<Device>,
    settings: Option<Res<LuisaDisplaySettings>>,
    winit_windows: NonSend<WinitWindows>,
    query: Query<(Entity, &Window)>,
) {
    let settings = settings.as_deref().copied().unwrap_or_default();
    for (entity, window) in query.iter() {
        let mut fields = FieldSet::new();
        let w = window.resolution.physical_width();
        let h = window.resolution.physical_height();
        let swapchain = device.create_swapchain(
            winit_windows.get_window(entity).unwrap(),
            &device.default_stream(),
            w,
            h,
            settings.allow_hdr,
            settings.vsync,
            settings.back_buffer_size,
        );
        let color_texture = device.create_tex2d::<Vec4<f32>>(swapchain.pixel_storage(), w, h, 1);
        let domain = StaticDomain::<2>::new(w, h);
        let mapping = domain.map_tex2d(color_texture.view(0));
        let color = fields.create_bind("display-color-final", mapping);

        commands.entity(entity).insert((
            LuisaSwapchain(swapchain),
            DisplayTexture {
                fields,
                domain,
                color,
                color_texture,
            },
        ));
    }
}

pub fn present_swapchain(
    device: Res<Device>,
    query: Query<(&LuisaSwapchain, &DisplayTexture), With<Window>>,
) {
    let scope = device.default_stream().scope();
    for (swapchain, display) in query.iter() {
        scope.present(swapchain, &display.color_texture);
    }
}

#[derive(Debug, Copy, Clone, Reflect, Resource)]
pub struct LuisaDisplaySettings {
    pub allow_hdr: bool,
    pub vsync: bool,
    pub back_buffer_size: u32,
}
impl Default for LuisaDisplaySettings {
    fn default() -> Self {
        Self {
            allow_hdr: false,
            vsync: false,
            back_buffer_size: 3,
        }
    }
}

pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_display)
            .add_systems(PostUpdate, present_swapchain);
    }
}
