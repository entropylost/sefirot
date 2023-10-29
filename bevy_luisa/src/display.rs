use super::*;
use bevy::winit::WinitWindows;
use luisa::lang::types::vector::Vec4;

#[derive(Deref, Component)]
pub struct DisplayTexture(pub Tex2d<Vec4<f32>>);

#[derive(Deref, Component)]
pub struct LuisaSwapchain(pub Swapchain);

#[derive(Deref, DerefMut, Resource, Copy, Clone, PartialEq, Debug)]
pub struct ClearColor(pub Vec4<f32>);
impl Default for ClearColor {
    fn default() -> Self {
        Self(Vec4::new(0.0, 0.0, 0.0, 1.0))
    }
}

pub fn setup_display(
    mut commands: Commands,
    device: Res<LuisaDevice>,
    settings: Res<LuisaDisplaySettings>,
    winit_windows: NonSend<WinitWindows>,
    query: Query<(Entity, &Window)>,
) {
    for (entity, window) in query.iter() {
        let swapchain = device.create_swapchain(
            winit_windows.get_window(entity).unwrap(),
            &device.default_stream(),
            window.resolution.physical_width(),
            window.resolution.physical_height(),
            settings.allow_hdr,
            settings.vsync,
            settings.back_buffer_size,
        );
        let display = device.create_tex2d::<Vec4<f32>>(
            swapchain.pixel_storage(),
            window.resolution.physical_width(),
            window.resolution.physical_height(),
            1,
        );

        commands
            .entity(entity)
            .insert((LuisaSwapchain(swapchain), DisplayTexture(display)));
    }
}

pub fn update_display(
    device: Res<LuisaDevice>,
    settings: Res<LuisaDisplaySettings>,
    winit_windows: NonSend<WinitWindows>,
    mut query: Query<(Entity, &Window, &mut LuisaSwapchain, &mut DisplayTexture), Changed<Window>>,
) {
    for (entity, window, mut luisa_swapchain, mut display_texture) in query.iter_mut() {
        let swapchain = device.create_swapchain(
            winit_windows.get_window(entity).unwrap(),
            &device.default_stream(),
            window.resolution.physical_width(),
            window.resolution.physical_height(),
            settings.allow_hdr,
            settings.vsync,
            settings.back_buffer_size,
        );
        let display = device.create_tex2d::<Vec4<f32>>(
            luisa_swapchain.pixel_storage(),
            window.resolution.physical_width(),
            window.resolution.physical_height(),
            1,
        );
        luisa_swapchain.0 = swapchain;
        display_texture.0 = display;
    }
}

#[kernel]
fn clear_display_kernel() {
    |display: Tex2dVar<Vec4<f32>>, clear_color: Expr<Vec4<f32>>| {
        display.write(dispatch_id().xy(), clear_color);
    }
}

pub fn present_swapchain_and_clear(
    device: Res<LuisaDevice>,
    clear_color: Option<Res<ClearColor>>,
    query: Query<(&LuisaSwapchain, &DisplayTexture, &Window)>,
) {
    let scope = device.default_stream().scope();
    for (swapchain, display, window) in query.iter() {
        scope.present(swapchain, display);
        scope.submit([clear_display_kernel.dispatch_async(
            [
                window.resolution.physical_width(),
                window.resolution.physical_height(),
                1,
            ],
            &display.0,
            &clear_color.as_deref().copied().unwrap_or_default().0,
        )]);
    }
}

#[derive(Debug, Copy, Clone, Default, Reflect, Resource)]
pub struct LuisaDisplaySettings {
    pub allow_hdr: bool,
    pub vsync: bool,
    pub back_buffer_size: u32,
}

pub struct LuisaDisplayPlugin {
    pub allow_hdr: bool,
    pub vsync: bool,
    pub back_buffer_size: u32,
}
impl Default for LuisaDisplayPlugin {
    fn default() -> Self {
        Self {
            allow_hdr: false,
            vsync: false,
            back_buffer_size: 3,
        }
    }
}

impl Plugin for LuisaDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LuisaDisplaySettings {
            allow_hdr: self.allow_hdr,
            vsync: self.vsync,
            back_buffer_size: self.back_buffer_size,
        })
        .add_systems(Startup, setup_display)
        .add_systems(PostUpdate, present_swapchain_and_clear);
    }
}
