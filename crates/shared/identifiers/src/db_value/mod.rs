//! Database-value abstraction shared between repository code and the
//! identifier crate.
//!
//! [`DbValue`] carries a NULL marker per scalar type rather than one untyped
//! NULL so that a bound parameter keeps its SQL type when the Rust value is
//! absent; the conversion traits and [`JsonRow`] are built on that invariant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod from_value;
mod to_value;
mod value;

pub use from_value::FromDbValue;
pub use to_value::ToDbValue;
pub use value::{DbValue, JsonRow, parse_database_datetime};
