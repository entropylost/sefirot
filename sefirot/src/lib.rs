// This feature is basically guaranteed to be stablized. If the nightly dependency is an issue, just switch to `bevy`'s SyncCell.
#![feature(exclusive_wrapper)]
#![feature(duration_millis_float)]
#![allow(clippy::type_complexity)]

extern crate self as sefirot;

pub mod domain;
pub mod element;
pub mod field;
pub mod graph;
pub mod kernel;
pub mod mapping;
pub mod utils;

#[cfg(test)]
mod tests;

pub use luisa_compute as luisa;
pub use sefirot_macro::{track, track_nc, tracked, tracked_nc};

mod internal_prelude {
    pub use luisa::prelude::*;
    pub use luisa_compute as luisa;

    pub use crate::element::{Context, Element};
    pub use crate::field::{Access, Field, FieldId, FieldIndex};
    pub use crate::mapping::Mapping;
    pub use crate::utils::Paradox;
}

pub mod prelude {
    pub use luisa::prelude::*;
    pub use luisa_compute as luisa;

    pub use crate::domain::Domain;
    pub use crate::element::{AsKernelContext, Element};
    pub use crate::field::set::FieldSet;
    pub use crate::field::{AField, EField, Field, VField};
    pub use crate::graph::{AsNodes, CopyExt};
    pub use crate::kernel::Kernel;
    pub use crate::{track, tracked};
}

pub mod ext_prelude {
    pub use crate::domain::{DomainImpl, KernelDispatch};
    pub use crate::element::Context;
    pub use crate::graph::NodeConfigs;
    pub use crate::kernel::KernelContext;
    pub use crate::mapping::{EMapping, Mapping, VMapping};
    pub use crate::prelude::*;
    pub use crate::{track_nc, tracked_nc};
}
