use sefirot::domain::kernel::KernelSignature;
use sefirot::prelude::{EmanationType, Kernel};
use std::ops::Deref;
use std::sync::OnceLock;

pub use bevy_sefirot_macro::init_kernel;

pub mod prelude {
    pub use bevy_luisa::{LuisaDevice, LuisaPlugin};
    pub use bevy_sefirot_macro::init_kernel;
    pub use sefirot::prelude::*;
    pub use {bevy_luisa, sefirot};
}

pub struct KernelCell<T: EmanationType, S: KernelSignature, A = ()>(OnceLock<Kernel<T, S, A>>);

impl<T: EmanationType, S: KernelSignature, A> Deref for KernelCell<T, S, A> {
    type Target = Kernel<T, S, A>;
    fn deref(&self) -> &Self::Target {
        self.0.get().unwrap()
    }
}
impl<T: EmanationType, S: KernelSignature, A> KernelCell<T, S, A> {
    pub const fn default() -> Self {
        Self(OnceLock::new())
    }
    pub fn init(&self, kernel: Kernel<T, S, A>) {
        self.0.set(kernel).ok().unwrap();
    }
}
