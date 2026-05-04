//! Top-level build orchestration: drives CSS organisation and post-build
//! validation, with progress reporting via `indicatif`.

mod orchestrator;
mod steps;
mod validation;

pub use orchestrator::{BuildError, BuildMode, BuildOrchestrator};
