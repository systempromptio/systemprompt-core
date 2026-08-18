//! `admin users api-key` command tree.
//!
//! Mints, lists, and revokes `sp-live-` personal access tokens directly
//! against the database — the same `ApiKeyService` path the gateway's
//! browser-consent exchange lands on, usable before any admin HTTP session
//! or external identity provider exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use serde::Serialize;
use std::sync::Arc;
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_users::{ApiKeyService, IssueApiKeyParams, UserRepository};

use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Subcommand)]
pub enum ApiKeyCommands {
    #[command(about = "Issue a personal access token; the secret is printed once")]
    Issue(IssueArgs),

    #[command(about = "List a user's API keys")]
    List(ListArgs),

    #[command(about = "Revoke an API key")]
    Revoke(RevokeArgs),
}

#[derive(Debug, Args)]
pub struct IssueArgs {
    #[arg(long)]
    pub user: String,

    #[arg(long)]
    pub name: String,

    #[arg(long, value_parser = parse_rfc3339)]
    pub expires: Option<DateTime<Utc>>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub user: String,
}

#[derive(Debug, Args)]
pub struct RevokeArgs {
    #[arg(long)]
    pub user: String,

    #[arg(long)]
    pub id: String,
}

fn parse_rfc3339(raw: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("expected an RFC 3339 timestamp: {e}"))
}

#[derive(Debug, Serialize)]
struct IssuedKeyOutput {
    id: ApiKeyId,
    user_id: UserId,
    name: String,
    expires_at: Option<DateTime<Utc>>,
    secret: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct KeyRow {
    id: ApiKeyId,
    name: String,
    key_prefix: String,
    created_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

pub(super) async fn execute(cmd: ApiKeyCommands, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let service = ApiKeyService::new(Arc::new(UserRepository::new(&pool)?));
    match cmd {
        ApiKeyCommands::Issue(args) => issue(&service, args).await,
        ApiKeyCommands::List(args) => list(&service, &args).await,
        ApiKeyCommands::Revoke(args) => revoke(&service, &args).await,
    }
}

async fn issue(service: &ApiKeyService, args: IssueArgs) -> Result<CommandOutput> {
    if args.name.trim().is_empty() {
        return Err(anyhow!("Key name cannot be empty"));
    }
    let user_id = UserId::new(args.user);
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &user_id,
            name: &args.name,
            expires_at: args.expires,
        })
        .await?;
    let output = IssuedKeyOutput {
        id: issued.record.id.clone(),
        user_id: issued.record.user_id.clone(),
        name: issued.record.name.clone(),
        expires_at: issued.record.expires_at,
        secret: issued.secret,
        message: "Store the secret now — it is shown only once".to_owned(),
    };
    Ok(CommandOutput::card_value("API Key Issued", &output))
}

async fn list(service: &ApiKeyService, args: &ListArgs) -> Result<CommandOutput> {
    let user_id = UserId::new(args.user.clone());
    let rows: Vec<KeyRow> = service
        .list_for_user(&user_id)
        .await?
        .into_iter()
        .map(|k| KeyRow {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
            expires_at: k.expires_at,
            revoked_at: k.revoked_at,
        })
        .collect();
    Ok(CommandOutput::card_value("API Keys", &rows))
}

async fn revoke(service: &ApiKeyService, args: &RevokeArgs) -> Result<CommandOutput> {
    let user_id = UserId::new(args.user.clone());
    let key_id = ApiKeyId::new(args.id.clone());
    let revoked = service.revoke(&key_id, &user_id).await?;
    if revoked {
        #[derive(Debug, Serialize)]
        struct RevokedOutput {
            id: ApiKeyId,
            message: String,
        }
        let output = RevokedOutput {
            id: key_id,
            message: "API key revoked".to_owned(),
        };
        Ok(CommandOutput::card_value("API Key Revoked", &output))
    } else {
        Err(anyhow!(
            "API key was not found for that user or is already revoked"
        ))
    }
}
