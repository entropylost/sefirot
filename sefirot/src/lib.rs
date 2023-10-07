// See commit 058f099b2e05089a498b9fe881b1f0ee10f847fd for a version that doesn't need these.
#![feature(unboxed_closures)]
#![feature(fn_traits)]

pub mod accessor;
pub mod domain;
pub mod element;
pub mod emanation;
pub mod graph;
pub mod ops;

pub use {luisa_compute as luisa, sefirot_macro as macros};

pub mod prelude {
    pub use crate::accessor::Accessor;
    pub use crate::domain::Domain;
    pub use crate::element::Element;
    pub use crate::emanation::{Emanation, EmanationType, Field};
    pub use luisa::prelude::*;
    pub use luisa_compute as luisa;
    pub use sefirot_macro::{track, tracked, Structure};
}
