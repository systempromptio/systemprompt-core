//! Top-level build orchestration: drives CSS organisation and post-build
//! validation, with progress reporting via `indicatif`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod orchestrator;
mod steps;
mod validation;

pub use orchestrator::{BuildError, BuildMode, BuildOrchestrator};
