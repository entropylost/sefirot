use sefirot::domain::kernel::KernelSignature;
use sefirot::prelude::{Domain, Kernel};
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

pub struct KernelCell<D: Domain, S: KernelSignature>(OnceLock<Kernel<D, S>>);

impl<D: Domain, S: KernelSignature> Deref for KernelCell<D, S> {
    type Target = Kernel<D, S>;
    fn deref(&self) -> &Self::Target {
        self.0.get().unwrap()
    }
}
impl<D: Domain, S: KernelSignature> KernelCell<D, S> {
    pub const fn default() -> Self {
        Self(OnceLock::new())
    }
    pub fn init(&self, kernel: Kernel<D, S>) {
        self.0.set(kernel).ok().unwrap();
    }
}
