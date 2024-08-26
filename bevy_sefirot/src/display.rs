use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RawHandleWrapper};
use bevy::winit::WinitWindows;
use luisa::lang::types::vector::{Vec2, Vec4};
use sefirot::mapping::buffer::StaticDomain;

use super::prelude::*;

#[derive(Component)]
pub struct DisplayTexture {
    /// The color texture for the display. Note that the format is not guaranteed,
    /// so this should not be used for intermediate calculations.
    pub color: VEField<Vec4<f32>, Vec2<u32>>,
    /// The actual texture that will be displayed.
    pub color_texture: Tex2d<Vec4<f32>>,
    /// The domain of the screen.
    // TODO: Implement resizing.
    pub domain: StaticDomain<2>,
    _fields: FieldSet,
}

#[derive(Component, Debug, Copy, Clone, PartialEq, Eq)]
pub struct LuisaWindow;

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
    settings: Option<Res<LuisaDisplaySettings>>,
    winit_windows: NonSend<WinitWindows>,
    query: Query<(Entity, &Window), With<LuisaWindow>>,
) {
    let settings = settings.as_deref().copied().unwrap_or_default();
    for (entity, window) in query.iter() {
        let mut fields = FieldSet::new();
        let w = window.resolution.physical_width();
        let h = window.resolution.physical_height();
        let swapchain = DEVICE.create_swapchain(
            &**winit_windows.get_window(entity).unwrap(),
            &DEVICE.default_stream(),
            w,
            h,
            settings.allow_hdr,
            settings.vsync,
            settings.back_buffer_size,
        );
        let color_texture = DEVICE.create_tex2d::<Vec4<f32>>(swapchain.pixel_storage(), w, h, 1);
        let domain = StaticDomain::<2>::new(w, h);
        let mapping = domain.map_tex2d(color_texture.view(0));
        let color = fields.create_bind("display-color-swapchain", mapping);

        commands
            .entity(entity)
            .insert((
                LuisaSwapchain(swapchain),
                DisplayTexture {
                    _fields: fields,
                    domain,
                    color,
                    color_texture,
                },
            ))
            .remove::<RawHandleWrapper>();
    }
}

pub fn setup_primary(mut commands: Commands, query: Query<Entity, With<PrimaryWindow>>) {
    for entity in query.iter() {
        commands.entity(entity).insert(LuisaWindow);
    }
}

// TODO: Make this run in parallel with the rest of the things using a separate stream and the ComputeTaskPool.
pub fn present_swapchain(query: Query<(&LuisaSwapchain, &DisplayTexture), With<Window>>) {
    let scope = DEVICE.default_stream().scope();
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

pub struct DisplayPlugin {
    activate_primary: bool,
}
impl Default for DisplayPlugin {
    fn default() -> Self {
        Self {
            activate_primary: true,
        }
    }
}

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_display)
            .add_systems(PostUpdate, present_swapchain);
        if self.activate_primary {
            app.add_systems(Startup, setup_primary.before(setup_display));
        }
    }
}
