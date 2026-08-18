//! Data model for evaluation runs, cases, results, rubrics, and sampling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod case;
mod result;
mod rubric;
mod run;
mod sampling;

pub use case::{CanonicalMessage, CanonicalPrompt, EvalCase, NewCaseParams};
pub use result::{EvalResult, NewResultParams, Verdict};
pub use rubric::{DimensionScore, JudgeVerdict, Rubric, RubricDimension};
pub use run::{EvalRun, EvalRunKind, EvalRunStatus, NewRunParams, TriggerSource};
pub use sampling::{SampleFilter, SampleMode, SampledRequest};
