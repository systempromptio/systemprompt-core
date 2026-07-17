//! Public user and session summary projections.
//!
//! [`UserSummary`] and [`SessionSummary`] are read-only DTOs returned by
//! the public API; they carry no credentials or internal state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod summary;

pub use summary::{SessionSummary, UserSummary};
