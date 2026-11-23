//! `systemprompt-brain-entry` — the brain entry surface.
//!
//! The `http` feature carries the capability's routes: fact reads, search,
//! person-entered facts, fact governance, dispute resolution and the proposal
//! queue under `/api/v1/brain`, and the recall simulator under
//! `/api/v1/admin/brain`. [`extension::BrainHttpExtension`] contributes both
//! route sets to the composition, so the API server does not name this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extension;
#[cfg(feature = "http")]
pub mod http;
