//! `admin users api-key issue`.
//!
//! `--if-absent` makes issuance idempotent per user and key name: a live key
//! with that name that does not expire within `--renew-before-days` is
//! reported (`issued: false`) instead of minting another. `--secret-file`
//! writes the secret to a `0600` file so it never reaches stdout.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, TimeDelta, Utc};
use clap::Args;
use serde::Serialize;
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_users::{
    ApiKeyLimits, ApiKeyService, EnsuredApiKey, IssueApiKeyParams, NewApiKey, UserApiKey,
};

use super::{parse_rfc3339, parse_scope_binding};
use crate::shared::CommandOutput;

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

    #[arg(long)]
    pub if_absent: bool,

    #[arg(long, requires = "if_absent", default_value_t = 0)]
    pub renew_before_days: u32,

    #[arg(long, value_name = "PATH")]
    pub secret_file: Option<PathBuf>,
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

#[derive(Debug, Serialize)]
struct IssuedKeyOutput {
    id: ApiKeyId,
    user_id: UserId,
    name: String,
    key_prefix: String,
    expires_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    limits: ApiKeyLimits,
    scopes: Vec<ScopeBinding>,
    issued: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_file: Option<PathBuf>,
    message: String,
}

pub(super) async fn execute(service: &ApiKeyService, args: IssueArgs) -> Result<CommandOutput> {
    if args.name.trim().is_empty() {
        return Err(anyhow!("Key name cannot be empty"));
    }
    let limits = args.limits();
    let params = IssueApiKeyParams {
        user_id: &args.user,
        name: &args.name,
        expires_at: args.expires,
        limits: &limits,
        scopes: &args.scopes,
        purpose: None,
    };
    let ensured = if args.if_absent {
        let renew_before = TimeDelta::days(i64::from(args.renew_before_days));
        service.ensure(params, renew_before).await?
    } else {
        EnsuredApiKey::Issued(service.issue(params).await?)
    };
    match ensured {
        EnsuredApiKey::Existing(record) => Ok(existing_output(record)),
        EnsuredApiKey::Issued(issued) => issued_output(issued, args.secret_file.as_deref()),
    }
}

fn existing_output(record: UserApiKey) -> CommandOutput {
    let output = IssuedKeyOutput {
        id: record.id,
        user_id: record.user_id,
        name: record.name,
        key_prefix: record.key_prefix,
        expires_at: record.expires_at,
        limits: record.limits,
        scopes: record.scopes,
        issued: false,
        secret: None,
        secret_file: None,
        message: "A live key with this name already exists; nothing was issued".to_owned(),
    };
    CommandOutput::card_value("API Key Exists", &output)
}

fn issued_output(issued: NewApiKey, secret_file: Option<&Path>) -> Result<CommandOutput> {
    let (secret, secret_file, message) = match secret_file {
        Some(path) => {
            write_secret_file(path, &issued.secret)?;
            (
                None,
                Some(path.to_path_buf()),
                "Secret written to the secret file (mode 0600)".to_owned(),
            )
        },
        None => (
            Some(issued.secret),
            None,
            "Store the secret now — it is shown only once".to_owned(),
        ),
    };
    let record = issued.record;
    let output = IssuedKeyOutput {
        id: record.id,
        user_id: record.user_id,
        name: record.name,
        key_prefix: record.key_prefix,
        expires_at: record.expires_at,
        limits: record.limits,
        scopes: record.scopes,
        issued: true,
        secret,
        secret_file,
        message,
    };
    Ok(CommandOutput::card_value("API Key Issued", &output))
}

fn write_secret_file(path: &Path, secret: &str) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        let mut file = options
            .open(path)
            .with_context(|| format!("open secret file {}", path.display()))?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("restrict secret file {}", path.display()))?;
        file.write_all(secret.as_bytes())
            .with_context(|| format!("write secret file {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let mut file = options
            .open(path)
            .with_context(|| format!("open secret file {}", path.display()))?;
        file.write_all(secret.as_bytes())
            .with_context(|| format!("write secret file {}", path.display()))?;
    }
    Ok(())
}
