//! `admin config catalog` — edit the profile's provider registry
//! (`profile.providers`).
//!
//! Mutates the typed `ProviderRegistry` on the profile — adding or removing
//! providers and the models each provider serves — then revalidates the whole
//! profile before writing it back. This is how an instance declares a custom
//! provider such as `minimax` (its wire protocol, endpoint, credential, and
//! model catalog) without hand-editing YAML.

use std::collections::HashMap;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::Profile;
use systemprompt_models::profile::{ApiSurface, ProviderEntry, ProviderModel, WireProtocol};

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
    if matches!(command, CatalogCommands::Provider(ProviderCommands::List)) {
        return list_providers(config);
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let message = match command {
        CatalogCommands::Provider(ProviderCommands::List) => unreachable!("handled above"),
        CatalogCommands::Provider(ProviderCommands::Add(args)) => add_provider(&mut profile, args)?,
        CatalogCommands::Provider(ProviderCommands::Remove { name }) => {
            remove_provider(&mut profile, name)?
        },
        CatalogCommands::Model(ModelCommands::Add(args)) => add_model(&mut profile, args)?,
        CatalogCommands::Model(ModelCommands::Remove { provider, id }) => {
            remove_model(&mut profile, provider, id)?
        },
    };

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

fn add_provider(profile: &mut Profile, args: &ProviderAddArgs) -> Result<String> {
    // Preserve the existing model catalog when replacing a provider in place.
    let models = profile
        .providers
        .find_provider(&args.name)
        .map(|p| p.models.clone())
        .unwrap_or_default();
    let entry = ProviderEntry {
        name: ProviderId::new(&args.name),
        wire: parse_wire(&args.wire)?,
        surface: parse_surface(&args.surface)?,
        endpoint: args.endpoint.clone(),
        api_key_secret: SecretName::new(&args.api_key_secret),
        extra_headers: parse_headers(&args.headers)?,
        models,
    };
    profile
        .providers
        .providers
        .retain(|p| p.name.as_str() != args.name);
    profile.providers.providers.push(entry);
    Ok(format!(
        "Provider {} (wire {}, surface {}) added",
        args.name, args.wire, args.surface
    ))
}

fn remove_provider(profile: &mut Profile, name: &str) -> Result<String> {
    let before = profile.providers.providers.len();
    profile
        .providers
        .providers
        .retain(|p| p.name.as_str() != name);
    if profile.providers.providers.len() == before {
        bail!("No provider named {}", name);
    }
    Ok(format!("Provider {} removed", name))
}

fn add_model(profile: &mut Profile, args: &ModelAddArgs) -> Result<String> {
    let provider = profile
        .providers
        .providers
        .iter_mut()
        .find(|p| p.name.as_str() == args.provider)
        .ok_or_else(|| anyhow::anyhow!("No provider named {}", args.provider))?;
    let model = ProviderModel {
        id: ModelId::new(&args.id),
        aliases: args.aliases.iter().map(ModelId::new).collect(),
        upstream_model: args.upstream_model.clone(),
        pricing: systemprompt_models::services::ai::ModelPricing::default(),
        capabilities: systemprompt_models::services::ai::ModelCapabilities::default(),
        limits: systemprompt_models::services::ai::ModelLimits::default(),
    };
    provider.models.retain(|m| m.id.as_str() != args.id);
    provider.models.push(model);
    Ok(format!("Model {} added to {}", args.id, args.provider))
}

fn remove_model(profile: &mut Profile, provider_name: &str, id: &str) -> Result<String> {
    let provider = profile
        .providers
        .providers
        .iter_mut()
        .find(|p| p.name.as_str() == provider_name)
        .ok_or_else(|| anyhow::anyhow!("No provider named {}", provider_name))?;
    let before = provider.models.len();
    provider.models.retain(|m| m.id.as_str() != id);
    if provider.models.len() == before {
        bail!("No model with id {} under provider {}", id, provider_name);
    }
    Ok(format!("Model {} removed from {}", id, provider_name))
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
