//! Router state for the managed-resource admin surface: the application
//! context, which owns the managed repository the handlers read and write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::FromRef;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone)]
pub struct ManagedState {
    ctx: AppContext,
}

impl ManagedState {
    pub const fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    pub const fn ctx(&self) -> &AppContext {
        &self.ctx
    }
}

impl FromRef<ManagedState> for AppContext {
    fn from_ref(state: &ManagedState) -> Self {
        state.ctx.clone()
    }
}
