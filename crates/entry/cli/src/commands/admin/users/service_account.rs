//! `admin users service-account` command tree.
//!
//! `create` is idempotent on the email: re-running it returns the existing
//! service account (`created: false`), so a provisioning job can call it on
//! every deploy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};
use serde::Serialize;
use systemprompt_identifiers::UserId;
use systemprompt_users::{NewServiceAccount, UserRepository};

use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Subcommand)]
pub enum ServiceAccountCommands {
    #[command(about = "Create a service account, or return the existing one with that email")]
    Create(CreateServiceAccountArgs),
}

#[derive(Debug, Args)]
pub struct CreateServiceAccountArgs {
    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub email: String,

    #[arg(long)]
    pub created_by: UserId,
}

#[derive(Debug, Serialize)]
struct ServiceAccountOutput {
    id: UserId,
    name: String,
    email: String,
    created: bool,
}

pub(super) async fn execute(
    cmd: ServiceAccountCommands,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    match cmd {
        ServiceAccountCommands::Create(args) => create(args, ctx).await,
    }
}

async fn create(args: CreateServiceAccountArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    if args.name.trim().is_empty() || args.email.trim().is_empty() {
        return Err(anyhow!("Service account name and email cannot be empty"));
    }
    let pool = ctx.db_pool().await?;
    let ensured = UserRepository::new(&pool)
        .ensure_service_account(NewServiceAccount {
            name: args.name.trim().to_owned(),
            email: args.email,
            created_by: args.created_by,
        })
        .await?;
    let title = if ensured.created {
        "Service Account Created"
    } else {
        "Service Account Exists"
    };
    let output = ServiceAccountOutput {
        id: ensured.user.id,
        name: ensured.user.name,
        email: ensured.user.email,
        created: ensured.created,
    };
    Ok(CommandOutput::card_value(title, &output))
}
