//! `admin config catalog` — edit the profile's provider registry
//! (`profile.providers`).
//!
//! Parses the operator's arguments into typed specs and delegates the registry
//! mutation to [`ProviderCatalogService`], then revalidates the whole profile
//! before writing it back. This is how an instance declares a custom provider
//! such as `minimax` (its wire protocol, endpoint, credential, and model
//! catalog) without hand-editing YAML.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_config::{ModelSpec, ProfileBootstrap, ProviderCatalogService, ProviderSpec};
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::profile::{ApiSurface, WireProtocol};

use super::profile_io::{load_profile, save_profile};
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
            apply(config, |profile| {
                ProviderCatalogService::upsert_provider(
                    &mut profile.providers,
                    provider_spec(args)?,
                );
                Ok(format!(
                    "Provider {} (wire {}, surface {}) added",
                    args.name, args.wire, args.surface
                ))
            })
            .await
        },
        CatalogCommands::Provider(ProviderCommands::Remove { name }) => {
            apply(config, |profile| {
                ProviderCatalogService::remove_provider(
                    &mut profile.providers,
                    &ProviderId::new(name),
                )?;
                Ok(format!("Provider {} removed", name))
            })
            .await
        },
        CatalogCommands::Model(ModelCommands::Add(args)) => {
            apply(config, |profile| {
                ProviderCatalogService::upsert_model(&mut profile.providers, model_spec(args))?;
                Ok(format!("Model {} added to {}", args.id, args.provider))
            })
            .await
        },
        CatalogCommands::Model(ModelCommands::Remove { provider, id }) => {
            apply(config, |profile| {
                ProviderCatalogService::remove_model(
                    &mut profile.providers,
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
    mutate: impl FnOnce(&mut systemprompt_models::Profile) -> Result<String>,
) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;
    let message = mutate(&mut profile)?;

    save_profile(&profile, profile_path)?;
    let outcome = super::reconcile::reconcile_authz(&profile, profile_path).await;

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
    let profile_path = ProfileBootstrap::get_path()?;
    let profile = load_profile(profile_path)?;
    let items: Vec<ListItem> = profile
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
