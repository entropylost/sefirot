use std::env::current_exe;
use std::path::PathBuf;

use bevy::prelude::*;
use sefirot::luisa::prelude::*;

#[derive(Resource, Deref)]
pub struct LuisaDevice(pub Device);

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, SystemSet)]
pub struct InitKernel;

pub struct LuisaPlugin {
    pub lib_path: PathBuf,
    pub device: String,
}
impl Default for LuisaPlugin {
    fn default() -> Self {
        Self {
            lib_path: current_exe().unwrap(),
            device: "cuda".to_string(),
        }
    }
}

impl Plugin for LuisaPlugin {
    fn build(&self, app: &mut App) {
        let lib_path = self.lib_path.clone();
        let device = self.device.clone();
        let ctx = Context::new(lib_path);
        let device = ctx.create_device(device);
        app.insert_resource(LuisaDevice(device));
    }
}
