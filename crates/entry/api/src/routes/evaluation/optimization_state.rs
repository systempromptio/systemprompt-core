//! Router state for the evaluation admin surface: the application context plus
//! the optimization orchestrator composed once from its repositories.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::FromRef;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::optimization::SkillOptimizationOrchestrator;

#[derive(Debug, Clone)]
pub struct OptimizationState {
    ctx: AppContext,
    orchestrator: Arc<SkillOptimizationOrchestrator>,
}

impl OptimizationState {
    pub fn new(ctx: AppContext) -> Self {
        let orchestrator = Arc::new(SkillOptimizationOrchestrator::new(
            ctx.managed_repository().as_ref().clone(),
            ctx.evaluation_repositories().as_ref().clone(),
        ));
        Self { ctx, orchestrator }
    }

    pub fn orchestrator(&self) -> &SkillOptimizationOrchestrator {
        &self.orchestrator
    }

    pub const fn ctx(&self) -> &AppContext {
        &self.ctx
    }
}

impl FromRef<OptimizationState> for AppContext {
    fn from_ref(state: &OptimizationState) -> Self {
        state.ctx.clone()
    }
}
