//! Declarative macros that generate the typed identifier and token newtypes.
//!
//! The two public macros — [`define_id!`] and [`define_token!`] — are
//! `#[macro_export]`ed at the crate root. This module exists to keep their
//! source files individually below the 300-line cohesion limit and to expose
//! the supporting helper macros under stable paths.

mod helpers;
mod id;
mod token;

pub use crate::{__define_id_common, __define_id_validated_conversions, define_id, define_token};
