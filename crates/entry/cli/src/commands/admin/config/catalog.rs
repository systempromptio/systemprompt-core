//! `admin config catalog` — edit the services provider registry
//! (`services/ai/providers.yaml`).
//!
//! Parses the operator's arguments into typed specs and delegates the registry
//! mutation to [`ProviderCatalogService`], validates the result against the
//! merged services config, then writes the one file back. The profile is never
//! touched: the catalog is implementation configuration shipped with the
//! deployment. This is how an instance declares a custom provider such as
//! `minimax` (its wire protocol, endpoint, credential, and model catalog)
//! without hand-editing YAML.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_config::{ModelSpec, ProviderCatalogService, ProviderSpec};
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::services::{ApiSurface, ProviderRegistry, WireProtocol};

use super::services_io::{
    booted_services, load_providers_file, merged_registry_after_edit, providers_relative, save_file,
};
use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};
use systemprompt_models::artifacts::ListItem;

#[derive(Debug, Subcommand)]
pub enum CatalogCommands {
    #[command(subcommand, about = "Manage registry providers")]
    Provider(ProviderCommands),

    #[command(subcommand, about = "Manage the models a provider serves")]
    Model(ModelCommands),
}

#[derive(Debug, Subcommand)]
pub enum ProviderCommands {
    #[command(about = "List declared providers")]
    List,
    #[command(about = "Add or replace a provider")]
    Add(ProviderAddArgs),
    #[command(about = "Remove a provider by name")]
    Remove {
        #[arg(long)]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    #[command(about = "Add or replace a model under a provider")]
    Add(ModelAddArgs),
    #[command(about = "Remove a model by id from a provider")]
    Remove {
        #[arg(long, help = "Provider that serves the model")]
        provider: String,
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct ProviderAddArgs {
    #[arg(long)]
    pub name: String,
    #[arg(
        long,
        help = "Wire codec: anthropic | openai-chat | openai-responses | gemini"
    )]
    pub wire: String,
    #[arg(
        long,
        help = "Client API surface: anthropic | openai | gemini | backend"
    )]
    pub surface: String,
    #[arg(long)]
    pub endpoint: String,
    #[arg(long)]
    pub api_key_secret: String,
    #[arg(long = "header", help = "Extra header as KEY=VALUE (repeatable)")]
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ModelAddArgs {
    #[arg(long, help = "Provider that serves this model")]
    pub provider: String,
    #[arg(long)]
    pub id: String,
    #[arg(long = "alias", help = "Model alias (repeatable)")]
    pub aliases: Vec<String>,
    #[arg(
        long,
        help = "Vendor-side model name to forward upstream (defaults to id)"
    )]
    pub upstream_model: Option<String>,
}

pub async fn execute(command: &CatalogCommands, config: &CliConfig) -> Result<()> {
    match command {
        CatalogCommands::Provider(ProviderCommands::List) => list_providers(config),
        CatalogCommands::Provider(ProviderCommands::Add(args)) => {
            apply(config, |registry| {
                ProviderCatalogService::upsert_provider(registry, provider_spec(args)?);
                Ok(format!(
                    "Provider {} (wire {}, surface {}) added",
                    args.name, args.wire, args.surface
                ))
            })
            .await
        },
        CatalogCommands::Provider(ProviderCommands::Remove { name }) => {
            apply(config, |registry| {
                ProviderCatalogService::remove_provider(registry, &ProviderId::new(name))?;
                Ok(format!("Provider {} removed", name))
            })
            .await
        },
        CatalogCommands::Model(ModelCommands::Add(args)) => {
            apply(config, |registry| {
                ProviderCatalogService::upsert_model(registry, model_spec(args))?;
                Ok(format!("Model {} added to {}", args.id, args.provider))
            })
            .await
        },
        CatalogCommands::Model(ModelCommands::Remove { provider, id }) => {
            apply(config, |registry| {
                ProviderCatalogService::remove_model(
                    registry,
                    &ProviderId::new(provider),
                    &ModelId::new(id),
                )?;
                Ok(format!("Model {} removed from {}", id, provider))
            })
            .await
        },
    }
}

async fn apply(
    config: &CliConfig,
    mutate: impl FnOnce(&mut ProviderRegistry) -> Result<String>,
) -> Result<()> {
    let mut file = load_providers_file()?;
    let before = file.content.providers.clone();
    let message = mutate(&mut file.content.providers)?;

    let merged = merged_registry_after_edit(&before, &file.content.providers)?;
    let services = booted_services()?;
    if let Some(gateway) = services.gateway_config() {
        gateway
            .validate(&merged)
            .map_err(|e| anyhow::anyhow!("gateway no longer validates after edit: {e}"))?;
    }

    save_file(&file, providers_relative())?;
    let source = file.path.display().to_string();
    let outcome =
        super::reconcile::reconcile_authz(services.gateway.as_ref(), &merged, &source).await;

    render_result(
        &CommandOutput::card_value(
            "Provider Registry Updated",
            &ConfigMutationOutput {
                field: "providers".to_owned(),
                message: super::reconcile::append_reconcile_notice(message, &outcome),
            },
        ),
        config,
    );
    Ok(())
}

fn parse_wire(raw: &str) -> Result<WireProtocol> {
    WireProtocol::from_tag(raw).ok_or_else(|| {
        anyhow::anyhow!(
            "invalid --wire '{raw}'; expected one of: anthropic, openai-chat, \
             openai-responses, gemini"
        )
    })
}

fn parse_surface(raw: &str) -> Result<ApiSurface> {
    ApiSurface::from_tag(raw).ok_or_else(|| {
        anyhow::anyhow!(
            "invalid --surface '{raw}'; expected one of: anthropic, openai, gemini, backend"
        )
    })
}

fn parse_headers(raw: &[String]) -> Result<HashMap<String, String>> {
    raw.iter()
        .map(|h| {
            h.split_once('=')
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .ok_or_else(|| anyhow::anyhow!("invalid --header '{h}'; expected KEY=VALUE"))
        })
        .collect()
}

fn provider_spec(args: &ProviderAddArgs) -> Result<ProviderSpec> {
    Ok(ProviderSpec {
        name: ProviderId::new(&args.name),
        wire: parse_wire(&args.wire)?,
        surface: parse_surface(&args.surface)?,
        endpoint: args.endpoint.clone(),
        api_key_secret: SecretName::new(&args.api_key_secret),
        extra_headers: parse_headers(&args.headers)?,
    })
}

fn model_spec(args: &ModelAddArgs) -> ModelSpec {
    ModelSpec {
        provider: ProviderId::new(&args.provider),
        id: ModelId::new(&args.id),
        aliases: args.aliases.iter().map(ModelId::new).collect(),
        upstream_model: args.upstream_model.clone(),
    }
}

fn list_providers(config: &CliConfig) -> Result<()> {
    let items: Vec<ListItem> = booted_services()?
        .providers
        .providers
        .iter()
        .map(|p| {
            let models: Vec<&str> = p.models.iter().map(|m| m.id.as_str()).collect();
            let row = format!(
                "{} [wire {} / surface {}] {} ({} models: {})",
                p.name.as_str(),
                p.wire,
                p.surface,
                p.endpoint,
                models.len(),
                models.join(", ")
            );
            ListItem::new(row, String::new(), String::new())
        })
        .collect();
    render_result(
        &CommandOutput::list(items).with_title("Provider Registry"),
        config,
    );
    Ok(())
}
