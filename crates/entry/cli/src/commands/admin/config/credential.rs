//! `admin config credential` — the catalog's credentials
//! (`services/ai/credentials.yaml`): who authenticates and who pays.
//!
//! A `platform` credential names the secret holding an API key or a Google
//! service-account document; a `personal` credential names none, and every
//! member stores their own key under its id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::{CredentialSpec, ProviderCatalogService};
use systemprompt_identifiers::{CredentialId, SecretName};
use systemprompt_manifest::services::catalog::CredentialEntry;

use super::catalog_edit::{apply, read_active, render_list};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum CredentialCommands {
    #[command(about = "List declared credentials")]
    List,
    #[command(about = "Add or replace a credential")]
    Add(CredentialAddArgs),
    #[command(about = "Remove a credential by id")]
    Remove {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct CredentialAddArgs {
    #[arg(long)]
    pub id: String,
    #[arg(
        long,
        help = "platform (a deployment secret) | personal (each member's own key)"
    )]
    pub kind: String,
    #[arg(long, help = "Secret name holding the key (platform credentials only)")]
    pub secret: Option<String>,
    #[arg(long, help = "Label shown to members in Settings")]
    pub label: Option<String>,
}

pub async fn execute(command: &CredentialCommands, config: &CliConfig) -> Result<()> {
    match command {
        CredentialCommands::List => {
            let document = read_active()?;
            let rows = document
                .credentials
                .iter()
                .map(|(id, entry)| {
                    let secret = entry
                        .secret
                        .as_ref()
                        .map_or_else(String::new, |s| format!(" secret {}", s.as_str()));
                    format!("{id} [{}]{secret}", entry.kind.as_str())
                })
                .collect();
            render_list(config, "Credentials", rows);
            Ok(())
        },
        CredentialCommands::Add(args) => {
            let spec = credential_spec(args)?;
            apply(config, "Credential Updated", |document, context| {
                ProviderCatalogService::upsert_credential(document, context, spec)?;
                Ok(format!("Credential {} ({}) saved", args.id, args.kind))
            })
            .await
        },
        CredentialCommands::Remove { id } => {
            apply(config, "Credential Removed", |document, context| {
                ProviderCatalogService::remove_credential(
                    document,
                    context,
                    &CredentialId::new(id),
                )?;
                Ok(format!("Credential {id} removed"))
            })
            .await
        },
    }
}

fn credential_spec(args: &CredentialAddArgs) -> Result<CredentialSpec> {
    let entry = match (args.kind.as_str(), args.secret.as_deref()) {
        ("platform", Some(secret)) => CredentialEntry {
            label: args.label.clone(),
            ..CredentialEntry::platform(SecretName::new(secret))
        },
        ("platform", None) => bail!("a platform credential needs --secret"),
        ("personal", None) => CredentialEntry::personal(args.label.clone()),
        ("personal", Some(_)) => {
            bail!("a personal credential names no secret: each member stores their own key")
        },
        (other, _) => bail!("invalid --kind '{other}'; expected platform or personal"),
    };
    Ok(CredentialSpec {
        id: CredentialId::new(&args.id),
        entry,
    })
}
