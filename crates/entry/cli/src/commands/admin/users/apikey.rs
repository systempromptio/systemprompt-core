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
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{ApiKeyId, ScopeDimension, UserId};
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_security::authz::{AuthzHookContext, NullAuditSink, SubjectProviderSet};
use systemprompt_users::{ApiKeyLimits, ApiKeyService, IssueApiKeyParams, UserRepository};

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
    pub user: UserId,

    #[arg(long)]
    pub name: String,

    #[arg(long, value_parser = parse_rfc3339)]
    pub expires: Option<DateTime<Utc>>,

    #[arg(long = "model", value_name = "MODEL_ID")]
    pub models: Vec<String>,

    #[arg(long, value_parser = clap::value_parser!(i64).range(0..))]
    pub budget_microdollars: Option<i64>,

    #[arg(long, value_parser = clap::value_parser!(i32).range(1..))]
    pub max_requests: Option<i32>,

    #[arg(long, value_parser = clap::value_parser!(i32).range(1..))]
    pub window_seconds: Option<i32>,

    #[arg(long = "scope", value_name = "DIMENSION=VALUE", value_parser = parse_scope_binding)]
    pub scopes: Vec<ScopeBinding>,
}

impl IssueArgs {
    #[must_use]
    pub fn limits(&self) -> ApiKeyLimits {
        ApiKeyLimits {
            model_allowlist: (!self.models.is_empty()).then(|| self.models.clone()),
            budget_microdollars: self.budget_microdollars,
            max_requests: self.max_requests,
            request_window_seconds: self.window_seconds,
        }
    }
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub user: UserId,
}

#[derive(Debug, Args)]
pub struct RevokeArgs {
    #[arg(long)]
    pub user: UserId,

    #[arg(long, value_parser = crate::shared::parse_api_key_id)]
    pub id: ApiKeyId,
}

#[derive(Debug, thiserror::Error)]
#[error("expected an RFC 3339 timestamp: {0}")]
struct Rfc3339Error(#[source] chrono::ParseError);

fn parse_rfc3339(raw: &str) -> Result<DateTime<Utc>, Rfc3339Error> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(Rfc3339Error)
}

#[derive(Debug, thiserror::Error)]
enum ScopeArgError {
    #[error("expected DIMENSION=VALUE")]
    MissingSeparator,
    #[error("scope value cannot be empty")]
    EmptyValue,
    #[error("invalid scope dimension: {0}")]
    Dimension(#[source] IdValidationError),
}

fn parse_scope_binding(raw: &str) -> Result<ScopeBinding, ScopeArgError> {
    let (dimension, value) = raw.split_once('=').ok_or(ScopeArgError::MissingSeparator)?;
    let value = value.trim();
    if value.is_empty() {
        return Err(ScopeArgError::EmptyValue);
    }
    Ok(ScopeBinding {
        dimension: ScopeDimension::try_new(dimension.trim()).map_err(ScopeArgError::Dimension)?,
        value: value.to_owned(),
    })
}

#[derive(Debug, Serialize)]
struct IssuedKeyOutput {
    id: ApiKeyId,
    user_id: UserId,
    name: String,
    expires_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    limits: ApiKeyLimits,
    scopes: Vec<ScopeBinding>,
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
    #[serde(flatten)]
    limits: ApiKeyLimits,
    scopes: Vec<ScopeBinding>,
}

pub(super) async fn execute(cmd: ApiKeyCommands, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let service = ApiKeyService::new(Arc::new(UserRepository::new(&pool)));
    match cmd {
        ApiKeyCommands::Issue(args) => {
            if !args.scopes.is_empty() {
                SubjectProviderSet::discover(&AuthzHookContext {
                    pool: pool.pool(),
                    sink: Arc::new(NullAuditSink),
                })
                .verify_scope_bindings(&args.user, &args.scopes)
                .await?;
            }
            issue(&service, args).await
        },
        ApiKeyCommands::List(args) => list(&service, &args).await,
        ApiKeyCommands::Revoke(args) => revoke(&service, &args).await,
    }
}

async fn issue(service: &ApiKeyService, args: IssueArgs) -> Result<CommandOutput> {
    if args.name.trim().is_empty() {
        return Err(anyhow!("Key name cannot be empty"));
    }
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &args.user,
            name: &args.name,
            expires_at: args.expires,
            limits: &args.limits(),
            scopes: &args.scopes,
        })
        .await?;
    let output = IssuedKeyOutput {
        id: issued.record.id.clone(),
        user_id: issued.record.user_id.clone(),
        name: issued.record.name.clone(),
        expires_at: issued.record.expires_at,
        limits: issued.record.limits,
        scopes: issued.record.scopes,
        secret: issued.secret,
        message: "Store the secret now — it is shown only once".to_owned(),
    };
    Ok(CommandOutput::card_value("API Key Issued", &output))
}

async fn list(service: &ApiKeyService, args: &ListArgs) -> Result<CommandOutput> {
    let rows: Vec<KeyRow> = service
        .list_for_user(&args.user)
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
            limits: k.limits,
            scopes: k.scopes,
        })
        .collect();
    Ok(CommandOutput::card_value("API Keys", &rows))
}

async fn revoke(service: &ApiKeyService, args: &RevokeArgs) -> Result<CommandOutput> {
    let revoked = service.revoke(&args.id, &args.user).await?;
    if revoked {
        #[derive(Debug, Serialize)]
        struct RevokedOutput {
            id: ApiKeyId,
            message: String,
        }
        let output = RevokedOutput {
            id: args.id.clone(),
            message: "API key revoked".to_owned(),
        };
        Ok(CommandOutput::card_value("API Key Revoked", &output))
    } else {
        Err(anyhow!(
            "API key was not found for that user or is already revoked"
        ))
    }
}
