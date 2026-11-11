//! `admin config secret migrate-git-credentials`: move Git source credentials
//! stored under a bare label to the name the runtime resolves.
//!
//! A Git source's `credential_reference` is a label, resolved only as the
//! custom secret `GIT_SOURCE__<HOST>__<LABEL>` (see `git_credential_key`) for
//! the host of the source's repository. For each registered Git source with a
//! label, a secret stored under the bare label is copied to that name. An
//! existing namespaced secret is never overwritten, and the bare secret is
//! removed only when every source using that label now has its namespaced copy.
//! Secret values are never printed. Without `--apply` the command only reports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use systemprompt_config::write_private_atomic;
use systemprompt_logging::CliService;
use systemprompt_marketplace::managed::{GitCredentialSource, ManagedRepository};
use systemprompt_runtime::managed::git_sources::git_credential_key;

use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct MigrateGitCredentialsArgs {
    #[arg(long, help = "Apply the renames (default: report only)")]
    pub apply: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    Renamed,
    AlreadyNamespaced,
    NoSecretFound,
    Conflict,
    UnsupportedRepository,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceCredentialReport {
    pub source: String,
    pub label: String,
    pub target: Option<String>,
    pub status: CredentialStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MigrateGitCredentialsOutput {
    pub applied: bool,
    pub sources: Vec<SourceCredentialReport>,
    pub removed_bare_labels: Vec<String>,
    pub message: String,
}

// JSON: the on-disk secrets document is edited by key, keeping every entry
// this migration does not own exactly as it was.
pub fn plan_renames(
    secrets: &mut Map<String, Value>,
    sources: &[GitCredentialSource],
) -> (Vec<SourceCredentialReport>, Vec<String>) {
    let mut reports = Vec::with_capacity(sources.len());
    let mut blocked: BTreeSet<&str> = BTreeSet::new();
    let mut used: BTreeSet<&str> = BTreeSet::new();
    for source in sources {
        let label = source.credential_reference.as_str();
        let target = git_credential_key(&source.repository, label);
        let bare = secrets.get(label).cloned();
        let status = match (&target, bare) {
            (None, _) => CredentialStatus::UnsupportedRepository,
            (Some(target), bare) if secrets.contains_key(target) => match bare {
                Some(bare) if secrets.get(target) != Some(&bare) => CredentialStatus::Conflict,
                _ => CredentialStatus::AlreadyNamespaced,
            },
            (Some(target), Some(bare)) => {
                secrets.insert(target.clone(), bare);
                CredentialStatus::Renamed
            },
            (Some(_), None) => CredentialStatus::NoSecretFound,
        };
        used.insert(label);
        if matches!(
            status,
            CredentialStatus::Conflict | CredentialStatus::UnsupportedRepository
        ) {
            blocked.insert(label);
        }
        reports.push(SourceCredentialReport {
            source: source.name.clone(),
            label: label.to_owned(),
            target,
            status,
        });
    }
    let removed: Vec<String> = used
        .difference(&blocked)
        .filter(|label| secrets.remove(**label).is_some())
        .map(|label| (*label).to_owned())
        .collect();
    (reports, removed)
}

pub fn migrate_file(
    secrets_file: &Path,
    sources: &[GitCredentialSource],
    apply: bool,
) -> Result<MigrateGitCredentialsOutput> {
    let content = std::fs::read_to_string(secrets_file)
        .with_context(|| format!("Failed to read secrets: {}", secrets_file.display()))?;
    // JSON: operator tooling edits the on-disk secrets document by key.
    let mut doc: Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse secrets: {}", secrets_file.display()))?;
    let object = doc.as_object_mut().ok_or_else(|| {
        anyhow!(
            "secrets file is not a JSON object: {}",
            secrets_file.display()
        )
    })?;
    let (sources, removed_bare_labels) = plan_renames(object, sources);
    let renamed = sources
        .iter()
        .filter(|r| r.status == CredentialStatus::Renamed)
        .count();
    if apply && (renamed > 0 || !removed_bare_labels.is_empty()) {
        let serialized = CliService::json_string(&doc).context("Failed to serialize secrets")?;
        write_private_atomic(secrets_file, serialized.as_bytes())
            .with_context(|| format!("Failed to write {}", secrets_file.display()))?;
    }
    let message = if apply {
        format!(
            "{renamed} credential(s) renamed, {} bare label(s) removed",
            removed_bare_labels.len()
        )
    } else {
        format!("{renamed} credential(s) would be renamed. Re-run with --apply.")
    };
    Ok(MigrateGitCredentialsOutput {
        applied: apply,
        sources,
        removed_bare_labels,
        message,
    })
}

pub(super) async fn execute(
    args: MigrateGitCredentialsArgs,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let secrets_file = super::secret::file_secrets_path(ctx)?;
    let pool = ctx.db_pool().await?;
    let sources = ManagedRepository::new(&pool)
        .git_sources_with_credentials()
        .await?;
    let output = migrate_file(&secrets_file, &sources, args.apply)?;
    Ok(CommandOutput::card_value("Git Source Credentials", &output))
}
