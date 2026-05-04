//! Database-value abstraction shared between repository code and the
//! identifier crate.
//!
//! - [`DbValue`] is a tagged union of every scalar SQL value plus a NULL marker
//!   per type, used to ferry values between Rust and the SQL driver.
//! - [`ToDbValue`] / [`FromDbValue`] convert Rust types to and from
//!   [`DbValue`].
//! - [`JsonRow`] is a `HashMap<String, serde_json::Value>` row container.
//! - [`parse_database_datetime`] coerces driver-provided JSON values into
//!   `DateTime<Utc>`.

mod from_value;
mod to_value;
mod value;

pub use from_value::FromDbValue;
pub use to_value::ToDbValue;
pub use value::{DbValue, JsonRow, parse_database_datetime};
