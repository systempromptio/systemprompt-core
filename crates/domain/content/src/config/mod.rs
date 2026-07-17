//! Content configuration lifecycle.
//!
//! Models the two-stage transition from raw config to a queryable content
//! index: [`ContentConfigValidated`] verifies sources and categories against
//! the filesystem, and [`ContentReady`] scans the validated sources to produce
//! parsed content keyed by slug and source, alongside [`LoadStats`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ready;
mod validated;

pub use ready::{ContentReady, LoadStats, ParsedContent};
pub use validated::{ContentConfigValidated, ContentSourceConfigValidated, ValidationResult};
