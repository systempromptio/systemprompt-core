//! `admin config provider` — the catalog's vendor accounts
//! (`services/ai/providers.yaml`): credential, headers, user agent and
//! whether the provider serves at all.
//!
//! A new provider is added with its first endpoint (`--wire`, `--url`), since
//! a provider without one does not resolve; further endpoints are edited with
//! `admin config endpoint`. `disable` takes every endpoint of the provider
//! out of every chain, everywhere.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::{EndpointSpec, ProviderCatalogService, ProviderSpec};
use systemprompt_identifiers::{CredentialId, EndpointId, ProviderId};

use super::catalog_edit::{apply, parse_headers, parse_wire, read_active, render_list};
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
    #[arg(long, help = "Endpoint id of the first endpoint (default `api`)")]
    pub endpoint: Option<String>,
    #[arg(
        long,
        help = "Wire of the first endpoint (required for a new provider)"
    )]
    pub wire: Option<String>,
    #[arg(long, help = "URL of the first endpoint (required for a new provider)")]
    pub url: Option<String>,
}

fn first_endpoint(args: &ProviderAddArgs) -> Result<Option<EndpointSpec>> {
    match (args.wire.as_deref(), args.url.as_deref()) {
        (Some(wire), Some(url)) => Ok(Some(EndpointSpec {
            provider: ProviderId::new(&args.id),
            id: EndpointId::new(args.endpoint.as_deref().unwrap_or("api")),
            wire: parse_wire(wire)?,
            url: url.to_owned(),
        })),
        (None, None) => Ok(None),
        _ => bail!("--wire and --url go together"),
    }
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
                first_endpoint: first_endpoint(args)?,
            };
            apply(config, "Provider Updated", |document, context| {
                if document.providers.get(spec.id.as_str()).is_none()
                    && spec.first_endpoint.is_none()
                {
                    bail!("a new provider needs its first endpoint: pass --wire and --url");
                }
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
                first_endpoint: None,
            },
        )?;
        Ok(format!("Provider {id} enabled = {enabled}"))
    })
    .await
}
