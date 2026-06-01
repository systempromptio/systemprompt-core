//! `admin config catalog` — edit the gateway's model catalog (`catalog.yaml`).
//!
//! Resolves the catalog file from the profile's `gateway.catalog` path, mutates
//! the typed `GatewayCatalog`, runs `GatewayCatalog::validate` (SSRF-guarded
//! endpoints, provider/model consistency), and writes it back. This is how an
//! instance adds a custom provider such as `minimax` without hand-editing YAML.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::Profile;
use systemprompt_models::profile::{
    GatewayCatalog, GatewayCatalogSource, GatewayModel, GatewayProvider,
};

use super::profile_io::{load_profile, profile_dir};
use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::shared::{CommandResult, render_result};

#[derive(Debug, Subcommand)]
pub enum CatalogCommands {
    #[command(subcommand, about = "Manage catalog providers")]
    Provider(ProviderCommands),

    #[command(subcommand, about = "Manage catalog models")]
    Model(ModelCommands),
}

#[derive(Debug, Subcommand)]
pub enum ProviderCommands {
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
    #[command(about = "Add or replace a model")]
    Add(ModelAddArgs),
    #[command(about = "Remove a model by id")]
    Remove {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct ProviderAddArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub endpoint: String,
    #[arg(long)]
    pub api_key_secret: String,
    #[arg(long = "header", help = "Extra header as KEY=VALUE (repeatable)")]
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ModelAddArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub provider: String,
    #[arg(long = "alias", help = "Model alias (repeatable)")]
    pub aliases: Vec<String>,
    #[arg(long)]
    pub upstream_model: Option<String>,
    #[arg(long)]
    pub display_name: Option<String>,
}

pub fn execute(command: &CatalogCommands, _config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let profile = load_profile(profile_path)?;
    let catalog_file = catalog_file_path(&profile, profile_path)?;
    let mut catalog = read_catalog(&catalog_file)?;

    let message = match command {
        CatalogCommands::Provider(ProviderCommands::Add(args)) => add_provider(&mut catalog, args)?,
        CatalogCommands::Provider(ProviderCommands::Remove { name }) => {
            remove_provider(&mut catalog, name)?
        },
        CatalogCommands::Model(ModelCommands::Add(args)) => add_model(&mut catalog, args)?,
        CatalogCommands::Model(ModelCommands::Remove { id }) => remove_model(&mut catalog, id)?,
    };

    catalog
        .validate()
        .map_err(|e| anyhow::anyhow!("catalog validation failed: {e}"))?;
    write_catalog(&catalog, &catalog_file)?;

    render_result(
        &CommandResult::text(ConfigMutationOutput {
            field: "catalog".to_owned(),
            message,
        })
        .with_title("Catalog Updated"),
    );
    Ok(())
}

fn catalog_file_path(profile: &Profile, profile_path: &str) -> Result<PathBuf> {
    let spec = profile
        .gateway
        .clone()
        .map(systemprompt_models::profile::GatewayState::into_spec)
        .ok_or_else(|| anyhow::anyhow!("profile has no gateway section"))?;
    match spec.catalog {
        Some(GatewayCatalogSource::Path { path }) => {
            if path.is_absolute() {
                Ok(path)
            } else {
                Ok(profile_dir(profile_path).join(path))
            }
        },
        Some(GatewayCatalogSource::Inline(_)) => {
            bail!("gateway catalog is inline; edit the profile directly")
        },
        None => {
            bail!("gateway has no catalog source; run `admin config gateway catalog-set` first")
        },
    }
}

fn read_catalog(path: &std::path::Path) -> Result<GatewayCatalog> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read catalog: {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse catalog: {}", path.display()))
}

fn write_catalog(catalog: &GatewayCatalog, path: &std::path::Path) -> Result<()> {
    let content = serde_yaml::to_string(catalog).context("Failed to serialize catalog")?;
    std::fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
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

fn add_provider(catalog: &mut GatewayCatalog, args: &ProviderAddArgs) -> Result<String> {
    let provider = GatewayProvider {
        name: ProviderId::new(&args.name),
        endpoint: args.endpoint.clone(),
        api_key_secret: SecretName::new(&args.api_key_secret),
        extra_headers: parse_headers(&args.headers)?,
    };
    catalog.providers.retain(|p| p.name.as_str() != args.name);
    catalog.providers.push(provider);
    Ok(format!("Provider {} added", args.name))
}

fn remove_provider(catalog: &mut GatewayCatalog, name: &str) -> Result<String> {
    let before = catalog.providers.len();
    catalog.providers.retain(|p| p.name.as_str() != name);
    if catalog.providers.len() == before {
        bail!("No provider named {}", name);
    }
    Ok(format!("Provider {} removed", name))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "uniform Result signature across catalog mutation handlers dispatched together"
)]
fn add_model(catalog: &mut GatewayCatalog, args: &ModelAddArgs) -> Result<String> {
    let model = GatewayModel {
        id: ModelId::new(&args.id),
        provider: ProviderId::new(&args.provider),
        aliases: args.aliases.iter().map(ModelId::new).collect(),
        display_name: args.display_name.clone(),
        upstream_model: args.upstream_model.clone(),
        pricing: None,
    };
    catalog.models.retain(|m| m.id.as_str() != args.id);
    catalog.models.push(model);
    Ok(format!("Model {} added", args.id))
}

fn remove_model(catalog: &mut GatewayCatalog, id: &str) -> Result<String> {
    let before = catalog.models.len();
    catalog.models.retain(|m| m.id.as_str() != id);
    if catalog.models.len() == before {
        bail!("No model with id {}", id);
    }
    Ok(format!("Model {} removed", id))
}
