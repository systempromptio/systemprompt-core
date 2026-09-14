//! Evaluation domain crate for systemprompt.io.
//!
//! Provides immutable evaluation data, supervised experiments, and read-only
//! sampling of the platform's AI request trace.
//!
//! All paid experiment inference is dispatched through the reservation-backed
//! experiment gateway; this crate intentionally has no legacy direct-inference
//! improvement loop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod campaigns;
pub mod error;
pub mod experiments;
pub mod extension;
pub mod models;
pub mod repository;
pub mod services;

pub use error::{EvaluationError, Result};
pub use extension::EvaluationExtension;
pub use models::{
    CanonicalMessage, CanonicalPrompt, EvalCase, NewCaseParams, SampleFilter, SampleMode,
    SampledRequest,
};
pub use repository::{EvalCaseRepository, EvalRepositories, SamplingRepository};
pub use services::SamplerService;
