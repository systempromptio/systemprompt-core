//! Evaluation domain crate for systemprompt.io.
//!
//! Closes the loop on the platform's AI request trace (`ai_requests` and its
//! satellite tables, owned by `systemprompt-ai`): samples production traffic,
//! scores it against rubrics with an LLM judge, and replays failures with a
//! repair hint so repaired trajectories are scored and linked to the results
//! they fix.
//!
//! Public surface is a typed [`EvaluationError`] boundary, repositories over
//! the `eval_*` tables, and the services composing them:
//! [`EvaluationService`], [`SamplerService`], [`JudgeService`],
//! [`ReplayService`], and [`AutoImproveLoop`]. Judge and replay inference go
//! through a [`DynAiProvider`](systemprompt_models::ai::DynAiProvider)
//! supplied by the composition layer; it must be the platform's auditing
//! implementation, which persists every request to `ai_requests`.
//!
//! Judge and replay requests are attributed with a job actor so sampling —
//! which excludes `actor_kind = 'job'` — never grades the framework's own
//! traffic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error;
pub mod extension;
pub mod models;
pub mod repository;
pub mod services;

pub use error::{EvaluationError, Result};
pub use extension::EvaluationExtension;
pub use models::{
    CanonicalMessage, CanonicalPrompt, DimensionScore, EvalCase, EvalResult, EvalRun, EvalRunKind,
    EvalRunStatus, JudgeVerdict, NewCaseParams, NewResultParams, NewRunParams, Rubric,
    RubricDimension, SampleFilter, SampledRequest, TriggerSource, Verdict,
};
pub use repository::{
    EvalCaseRepository, EvalJudgeCallRepository, EvalResultRepository, EvalRubricRepository,
    EvalRunRepository, SamplingRepository,
};
pub use services::{
    AutoImproveLoop, EvaluationService, JudgeService, LoopLimits, LoopReport, ReplayService,
    RunRequest, SamplerService,
};
