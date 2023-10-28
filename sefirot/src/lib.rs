// This feature is basically guaranteed to be stablized. If the nightly dependency is an issue, just switch to `bevy`'s SyncCell.
#![feature(exclusive_wrapper)]
#![allow(clippy::type_complexity)]

extern crate self as sefirot;

pub mod domain;
pub mod element;
pub mod emanation;
pub mod field;
pub mod graph;
mod ops;

pub use {luisa_compute as luisa, sefirot_macro as macros};

#[cfg(feature = "bevy")]
#[doc(hidden)]
pub use bevy_ecs as _bevy_ecs;

pub mod prelude {
    pub use crate::domain::kernel::Kernel;
    pub use crate::domain::Domain;
    pub use crate::element::Element;
    pub use crate::emanation::{Emanation, EmanationType};
    pub use crate::field::{EField, Field};
    pub use luisa::prelude::*;
    pub use luisa_compute as luisa;
    pub use sefirot_macro::{track, tracked, Structure};
}
