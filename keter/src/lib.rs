#![feature(exclusive_wrapper)]
#![feature(duration_millis_float)]

use std::sync::LazyLock;

use luisa_compute::runtime::Device;
pub use luisa_compute::*;

pub mod graph;
pub mod pixel_storage;
pub mod utils;

#[doc(hidden)]
pub use luisa_compute as _luisa;

pub static DEVICE: LazyLock<Device> = LazyLock::new(|| {
    let lib_path = std::env::current_exe().unwrap();
    let ctx = luisa_compute::Context::new(lib_path);
    ctx.create_device(luisa_compute::DeviceType::Cuda)
});

pub mod prelude {
    pub use keter_macro::{track, tracked};
    pub use luisa_compute;
    pub use luisa_compute::prelude::*;

    pub use super::DEVICE;
    pub use crate::graph::{AsNodes, CopyExt};
    pub use crate::pixel_storage::HasPixelStorage;
    pub use crate::utils::{Angle, Singleton};
}
