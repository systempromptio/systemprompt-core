//! Data model for golden cases and traffic sampling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod case;
mod sampling;

pub use case::{CanonicalMessage, CanonicalPrompt, EvalCase, NewCaseParams};
pub use sampling::{SampleFilter, SampleMode, SampledRequest};
