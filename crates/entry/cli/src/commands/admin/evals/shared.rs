//! Shared lookup and rendering helpers for the admin evals commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_evaluation::EvalRepositories;
use systemprompt_identifiers::UserId;

use crate::context::CommandContext;

pub(super) struct EvalContext {
    pub repositories: EvalRepositories,
    pub admin_id: UserId,
}

pub(super) async fn eval_context(ctx: &CommandContext) -> Result<EvalContext> {
    let app_context = ctx.app_context().await?;
    let repositories = EvalRepositories::new(app_context.db_pool())
        .context("Failed to create evaluation repositories")?;

    Ok(EvalContext {
        repositories,
        admin_id: app_context.system_admin().id().clone(),
    })
}
