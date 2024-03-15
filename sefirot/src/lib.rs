// This feature is basically guaranteed to be stablized. If the nightly dependency is an issue, just switch to `bevy`'s SyncCell.
#![feature(exclusive_wrapper)]
#![allow(clippy::type_complexity)]

extern crate self as sefirot;

pub mod domain;
pub mod element;
pub mod field;
pub mod graph;
pub mod kernel;
pub mod mapping;
pub mod utils;

#[cfg(feature = "bevy")]
#[doc(hidden)]
pub use bevy_ecs as _bevy_ecs;
pub use luisa_compute as luisa;

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

    pub use crate::domain::{Domain, IndexDomain};
    pub use crate::element::Element;
    pub use crate::field::set::FieldSet;
    pub use crate::field::{AField, EField, Field, VField};
    pub use crate::kernel::Kernel;
}

pub mod ext_prelude {
    pub use crate::domain::DispatchArgs;
    pub use crate::element::Context;
    pub use crate::graph::{AsNodes, NodeConfigs};
    pub use crate::kernel::KernelContext;
    pub use crate::mapping::{EMapping, Mapping, VMapping};
    pub use crate::prelude::*;
}
