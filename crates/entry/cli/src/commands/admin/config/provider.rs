//! `admin config provider` — the catalog's vendor accounts
//! (`services/ai/providers.yaml`): credential, headers, user agent and
//! whether the provider serves at all.
//!
//! `disable` takes every endpoint of the provider out of every chain,
//! everywhere; endpoints are edited with `admin config endpoint`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_config::{ProviderCatalogService, ProviderSpec};
use systemprompt_identifiers::{CredentialId, ProviderId};

use super::catalog_edit::{apply, parse_headers, read_active, render_list};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ProviderCommands {
    #[command(about = "List declared providers and their endpoints")]
    List,
    #[command(about = "Add or update a provider (unset options keep their stored value)")]
    Add(ProviderAddArgs),
    #[command(about = "Enable a provider")]
    Enable {
        #[arg(long)]
        id: String,
    },
    #[command(about = "Disable a provider everywhere")]
    Disable {
        #[arg(long)]
        id: String,
    },
    #[command(about = "Remove a provider by id")]
    Remove {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct ProviderAddArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long, help = "Credential id the provider authenticates with")]
    pub credential: String,
    #[arg(long)]
    pub display_name: Option<String>,
    #[arg(long, help = "User agent; `{install}` expands to the site name")]
    pub user_agent: Option<String>,
    #[arg(long = "header", help = "Extra header as KEY=VALUE (repeatable)")]
    pub headers: Vec<String>,
}

pub async fn execute(command: &ProviderCommands, config: &CliConfig) -> Result<()> {
    match command {
        ProviderCommands::List => {
            let document = read_active()?;
            let rows = document
                .providers
                .iter()
                .map(|(id, provider)| {
                    let endpoints: Vec<String> = provider
                        .endpoints
                        .iter()
                        .map(|(endpoint, e)| format!("{endpoint} ({} {})", e.wire, e.url))
                        .collect();
                    let state = if provider.enabled { "" } else { " [disabled]" };
                    format!(
                        "{id}{state} credential {}: {}",
                        provider.credential,
                        endpoints.join(", ")
                    )
                })
                .collect();
            render_list(config, "Providers", rows);
            Ok(())
        },
        ProviderCommands::Add(args) => {
            let spec = ProviderSpec {
                id: ProviderId::new(&args.id),
                credential: CredentialId::new(&args.credential),
                display_name: args.display_name.clone(),
                user_agent: args.user_agent.clone(),
                extra_headers: parse_headers(&args.headers)?,
                enabled: None,
            };
            apply(config, "Provider Updated", |document, context| {
                ProviderCatalogService::upsert_provider(document, context, spec)?;
                Ok(format!("Provider {} saved", args.id))
            })
            .await
        },
        ProviderCommands::Enable { id } => set_enabled(config, id, true).await,
        ProviderCommands::Disable { id } => set_enabled(config, id, false).await,
        ProviderCommands::Remove { id } => {
            apply(config, "Provider Removed", |document, context| {
                ProviderCatalogService::remove_provider(document, context, &ProviderId::new(id))?;
                Ok(format!("Provider {id} removed"))
            })
            .await
        },
    }
}

async fn set_enabled(config: &CliConfig, id: &str, enabled: bool) -> Result<()> {
    apply(config, "Provider Updated", |document, context| {
        let credential = document
            .providers
            .get(id)
            .map(|p| p.credential.clone())
            .ok_or_else(|| anyhow::anyhow!("provider '{id}' is not declared"))?;
        ProviderCatalogService::upsert_provider(
            document,
            context,
            ProviderSpec {
                id: ProviderId::new(id),
                credential,
                display_name: None,
                user_agent: None,
                extra_headers: std::collections::HashMap::new(),
                enabled: Some(enabled),
            },
        )?;
        Ok(format!("Provider {id} enabled = {enabled}"))
    })
    .await
}
