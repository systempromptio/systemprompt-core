//! `admin users restore` and `admin users legal-hold` commands.
//!
//! A restore reverses an archive no older than
//! `retention.archived_users_days`: the account signs in again and returns to
//! listings. Credentials revoked at archive time stay revoked. A legal hold
//! keeps an archive from ever being purged until it is released.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_users::{User, UserRepository, UserService};

use systemprompt_extension::cli::CommandOutput;
use super::types::UserDeletedOutput;

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

async fn resolve(host: &dyn KernelHost, reference: &str) -> Result<(UserService, User)> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let user_service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let user = user_service
        .find_by_reference_any_status(reference)
        .await?
        .ok_or_else(|| anyhow!("User not found: {reference}"))?;
    Ok((user_service, user))
}

pub(super) async fn execute(args: RestoreArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let window_days = crate::commands::admin::state::config(host)
        .await?
        .retention
        .archived_users_days;
    let (user_service, user) = resolve(host, &args.user).await?;
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
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let (user_service, user) = resolve(host, &args.user).await?;
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
