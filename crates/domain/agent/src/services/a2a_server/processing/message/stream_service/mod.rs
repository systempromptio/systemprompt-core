//! Streaming execution pipeline for inbound messages.
//!
//! [`StreamService`] runs the strategy-driven pipeline as an owned task that
//! streams text, tool, and completion events back to the caller and stops
//! when its cancellation token fires.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod execution;
mod messages;
mod pipeline;
pub mod synthesis;

use std::sync::Arc;

use crate::repository::execution::ExecutionStepRepository;
use crate::services::{ContextService, SkillService};
use systemprompt_models::ai::DynAiProvider;

pub struct StreamService {
    pub ai_service: DynAiProvider,
    pub context_service: ContextService,
    pub skill_service: Arc<SkillService>,
    pub execution_step_repo: Arc<ExecutionStepRepository>,
}

impl std::fmt::Debug for StreamService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamService")
            .field("context_service", &self.context_service)
            .field("skill_service", &self.skill_service)
            .finish_non_exhaustive()
    }
}
