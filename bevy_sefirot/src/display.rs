use super::prelude::*;
use bevy::prelude::*;
use bevy::winit::WinitWindows;
use luisa::lang::types::vector::Vec4;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Display;
impl EmanationType for Display {}

#[derive(Component)]
pub struct DisplayTexture {
    pub index: ArrayIndex2d<Display>,
    /// The color texture for the display. Note that the format is not guaranteed,
    /// so this should not be used for intermediate calculations.
    pub color: EField<Vec4<f32>, Display>,
    /// The actual texture that will be displayed.
    pub color_texture: Tex2d<Vec4<f32>>,
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
    device: Res<LuisaDevice>,
    settings: Option<Res<LuisaDisplaySettings>>,
    winit_windows: NonSend<WinitWindows>,
    query: Query<(Entity, &Window)>,
) {
    let settings = settings.as_deref().copied().unwrap_or_default();
    let de = Emanation::<Display>::new(&device);
    for (entity, window) in query.iter() {
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
        let index = de.create_index2d([w, h]);
        let color = *de
            .create_field("display-color-final")
            .bind_tex2d(index, color_texture.view(0));

        commands.entity(entity).insert((
            LuisaSwapchain(swapchain),
            DisplayTexture {
                index,
                color,
                color_texture,
            },
        ));
    }
    commands.insert_resource(de);
}

pub fn present_swapchain(
    device: Res<LuisaDevice>,
    query: Query<(&LuisaSwapchain, &DisplayTexture), Has<Window>>,
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

pub struct LuisaDisplayPlugin;

impl Plugin for LuisaDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_display)
            .add_systems(PostUpdate, present_swapchain);
    }
}
