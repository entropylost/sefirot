use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, ScheduleLabel)]
pub struct InitKernel;

pub fn init_kernel_system(world: &mut World) {
    world.run_schedule(InitKernel);
}

#[derive(Debug, Clone, Default)]
pub struct LuisaPlugin;

impl Plugin for LuisaPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(InitKernel)
            .add_systems(PostStartup, init_kernel_system);
    }
}
