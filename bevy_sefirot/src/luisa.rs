use std::env::current_exe;
use std::path::PathBuf;

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use sefirot::luisa::prelude::*;
use sefirot::luisa::DeviceType;

#[derive(Resource, Deref)]
pub struct LuisaDevice(pub Device);

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, ScheduleLabel)]
pub struct InitKernel;

pub fn init_kernel_system(world: &mut World) {
    world.run_schedule(InitKernel);
}

pub struct LuisaPlugin {
    pub lib_path: PathBuf,
    pub device: DeviceType,
}
impl Default for LuisaPlugin {
    fn default() -> Self {
        Self {
            lib_path: current_exe().unwrap(),
            device: DeviceType::Cuda,
        }
    }
}

impl Plugin for LuisaPlugin {
    fn build(&self, app: &mut App) {
        let lib_path = self.lib_path.clone();
        let device = self.device;
        let ctx = Context::new(lib_path);
        let device = ctx.create_device(device);
        app.insert_resource(LuisaDevice(device))
            .init_schedule(InitKernel)
            .add_systems(PostStartup, init_kernel_system);
    }
}
