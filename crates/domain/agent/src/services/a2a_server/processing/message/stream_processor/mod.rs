//! Streaming execution pipeline for inbound messages.
//!
//! [`StreamProcessor`] runs the strategy-driven pipeline as an owned task that
//! streams text, tool, and completion events back to the caller and stops
//! when its cancellation token fires.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod helpers;
mod processing;

use std::sync::Arc;

use crate::repository::execution::ExecutionStepRepository;
use crate::services::{ContextService, SkillService};
use systemprompt_models::AiProvider;

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct StreamProcessor {
    pub ai_service: Arc<dyn AiProvider>,
    pub context_service: ContextService,
    pub skill_service: Arc<SkillService>,
    pub execution_step_repo: Arc<ExecutionStepRepository>,
}
