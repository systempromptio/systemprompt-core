//! Typed payload structs carried by the event enums.
//!
//! Re-exports the [`a2a`] and [`system`] payload modules, whose structs are
//! flattened into the corresponding event variants.

pub mod a2a;
pub mod system;

pub use a2a::*;
pub use system::*;
