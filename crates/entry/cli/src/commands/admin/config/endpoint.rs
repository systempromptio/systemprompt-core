//! `admin config endpoint` — one wire plus one URL under a provider
//! (`services/ai/providers.yaml`, `providers.<id>.endpoints`).
//!
//! An upsert keeps the stored betas, region, price multiplier, hosting and
//! resilience; only the wire and URL are set here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};
use systemprompt_config::{EndpointSpec, ProviderCatalogService};
use systemprompt_identifiers::{EndpointId, ProviderId};
use systemprompt_manifest::services::catalog::EndpointRef;

use super::catalog_edit::{apply, parse_wire};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum EndpointCommands {
    #[command(about = "Add or update an endpoint of a provider")]
    Add(EndpointAddArgs),
    #[command(about = "Remove an endpoint (`provider/endpoint`)")]
    Remove {
        #[arg(long, help = "provider/endpoint")]
        endpoint: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct EndpointAddArgs {
    #[arg(long, help = "Provider id the endpoint belongs to")]
    pub provider: String,
    #[arg(long)]
    pub id: String,
    #[arg(long, help = "anthropic | openai-chat | openai-responses | gemini")]
    pub wire: String,
    #[arg(long)]
    pub url: String,
}

pub async fn execute(command: &EndpointCommands, config: &CliConfig) -> Result<()> {
    match command {
        EndpointCommands::Add(args) => {
            let spec = EndpointSpec {
                provider: ProviderId::new(&args.provider),
                id: EndpointId::new(&args.id),
                wire: parse_wire(&args.wire)?,
                url: args.url.clone(),
            };
            apply(config, "Endpoint Updated", |document, context| {
                ProviderCatalogService::upsert_endpoint(document, context, spec)?;
                Ok(format!("Endpoint {}/{} saved", args.provider, args.id))
            })
            .await
        },
        EndpointCommands::Remove { endpoint } => {
            let target = EndpointRef::parse(endpoint).map_err(|e| anyhow!(e))?;
            apply(config, "Endpoint Removed", |document, context| {
                ProviderCatalogService::remove_endpoint(document, context, &target)?;
                Ok(format!("Endpoint {endpoint} removed"))
            })
            .await
        },
    }
}
