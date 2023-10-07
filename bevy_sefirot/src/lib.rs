use sefirot::domain::kernel::KernelSignature;
use sefirot::prelude::{EmanationType, Kernel};
use std::ops::Deref;
use std::sync::OnceLock;

pub use bevy_sefirot_macro::kernel;

pub mod prelude {
    pub use bevy_luisa::{
        execute_luisa_commands_blocking, execute_luisa_commands_delayed,
        synchronize_luisa_commands, Compute, LuisaCommandExt, LuisaCommands, LuisaCommandsType,
        LuisaDevice, LuisaPlugin,
    };
    pub use sefirot::prelude::*;
    pub use {bevy_luisa, sefirot};
}

pub struct KernelCell<T: EmanationType, S: KernelSignature>(OnceLock<Kernel<T, S>>);

impl<T: EmanationType, S: KernelSignature> Deref for KernelCell<T, S> {
    type Target = Kernel<T, S>;
    fn deref(&self) -> &Self::Target {
        self.0.get().unwrap()
    }
}
impl<T: EmanationType, S: KernelSignature> KernelCell<T, S> {
    pub const fn default() -> Self {
        Self(OnceLock::new())
    }
    pub fn init(&self, kernel: Kernel<T, S>) {
        self.0.set(kernel).ok().unwrap();
    }
}
