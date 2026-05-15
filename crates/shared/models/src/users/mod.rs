//! Public user and session summary projections.
//!
//! [`UserSummary`] and [`SessionSummary`] are read-only DTOs returned by
//! the public API; they carry no credentials or internal state.

mod summary;

pub use summary::{SessionSummary, UserSummary};
