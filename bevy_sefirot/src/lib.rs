use sefirot::domain::kernel::KernelSignature;
use sefirot::prelude::Kernel;
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

pub struct KernelCell<S: KernelSignature>(OnceLock<Kernel<S>>);

impl<S: KernelSignature> Deref for KernelCell<S> {
    type Target = Kernel<S>;
    fn deref(&self) -> &Self::Target {
        self.0.get().unwrap()
    }
}
impl<S: KernelSignature> KernelCell<S> {
    pub const fn default() -> Self {
        Self(OnceLock::new())
    }
    pub fn init(&self, kernel: Kernel<S>) {
        self.0.set(kernel).ok().unwrap();
    }
}
