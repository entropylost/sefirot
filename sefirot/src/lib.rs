pub mod accessor;
pub mod domain;
pub mod element;
pub mod emanation;
pub mod graph;
mod ops;

pub use {luisa_compute as luisa, sefirot_macro as macros};

#[cfg(feature = "bevy")]
#[doc(hidden)]
pub use bevy_ecs as _bevy_ecs;

pub mod prelude {
    pub use crate::accessor::Accessor;
    pub use crate::domain::kernel::Kernel;
    pub use crate::domain::Domain;
    pub use crate::element::Element;
    pub use crate::emanation::{Emanation, EmanationType, Field};
    pub use luisa::prelude::*;
    pub use luisa_compute as luisa;
    pub use sefirot_macro::{track, tracked, Structure};
}
