//! Evaluation services: sampling, judging, replay, and the auto-improve loop.
//!
//! All inference goes through the supplied
//! [`DynAiProvider`](systemprompt_models::ai::DynAiProvider) with a job actor
//! so the trace sampler never selects the framework's own traffic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod evaluation_service;
mod judge;
mod loop_runner;
mod replay;
mod sampler;

pub use evaluation_service::{EvaluationService, RunRequest};
pub use judge::{JudgeService, JudgeSpec, JudgeTarget, ScoredVerdict};
pub use loop_runner::{AutoImproveLoop, LoopLimits, LoopReport};
pub use replay::ReplayService;
pub use sampler::SamplerService;
