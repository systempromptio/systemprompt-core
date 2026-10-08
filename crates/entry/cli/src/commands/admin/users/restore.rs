//! `admin users restore` and `admin users legal-hold` commands.
//!
//! A restore reverses an archive no older than
//! `retention.archived_users_days`: the account signs in again and returns to
//! listings. Credentials revoked at archive time stay revoked. A legal hold
//! keeps an archive from ever being purged until it is released.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_users::{User, UserAdminService, UserRepository, UserService};

use super::types::UserDeletedOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct RestoreArgs {
    #[arg(value_name = "USER_ID", help = "User id, email, or name")]
    pub user: String,
}

#[derive(Debug, Args)]
pub struct LegalHoldArgs {
    #[arg(value_name = "USER_ID", help = "User id, email, or name")]
    pub user: String,

    #[arg(long, help = "Release the hold instead of placing it")]
    pub release: bool,
}

async fn resolve(ctx: &CommandContext, reference: &str) -> Result<(UserService, User)> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let admin_service = UserAdminService::new(user_service.clone());
    let user = admin_service
        .find_user(reference)
        .await?
        .ok_or_else(|| anyhow!("User not found: {reference}"))?;
    Ok((user_service, user))
}

pub(super) async fn execute(args: RestoreArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let window_days = ctx
        .app_context()
        .await?
        .config()
        .retention
        .archived_users_days;
    let (user_service, user) = resolve(ctx, &args.user).await?;
    user_service.restore(&user.id, window_days).await?;
    let output = UserDeletedOutput {
        id: user.id.clone(),
        message: format!(
            "User '{}' restored; credentials revoked at archive time stay revoked",
            user.name
        ),
    };
    Ok(CommandOutput::card_value("User Restored", &output))
}

pub(super) async fn execute_legal_hold(
    args: LegalHoldArgs,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let (user_service, user) = resolve(ctx, &args.user).await?;
    user_service.set_legal_hold(&user.id, !args.release).await?;
    let output = UserDeletedOutput {
        id: user.id.clone(),
        message: format!(
            "Legal hold {} for '{}'",
            if args.release { "released" } else { "placed" },
            user.name
        ),
    };
    Ok(CommandOutput::card_value("Legal Hold", &output))
}
