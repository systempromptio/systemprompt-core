//! `admin config gateway` — edit the services gateway section
//! (`services/ai/gateway.yaml`): enable state, routing patterns, and the
//! default provider.
//!
//! Every mutation resolves the resulting spec and validates it against the
//! merged services provider registry, so a route or default-provider that names
//! a provider absent from the registry fails at the edit rather than at the
//! next boot. The gateway owns no catalog: providers and models live in
//! `services/ai/providers.yaml` (see `admin config catalog`). The profile is
//! never touched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::{
    GatewayConfigSpec, GatewayRoute, GatewayState, ProviderRegistry,
};

use super::services_io::{
    GatewayFile, booted_services, gateway_relative, load_gateway_file, save_file,
};
use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};
use systemprompt_models::artifacts::ListItem;

#[derive(Debug, Subcommand)]
pub enum GatewayCommands {
    #[command(about = "Enable the gateway")]
    Enable,

    #[command(about = "Disable the gateway")]
    Disable,

    #[command(subcommand, about = "Manage gateway routes")]
    Route(RouteCommands),

    #[command(
        subcommand,
        about = "Manage the default provider (catch-all fallback route)"
    )]
    DefaultProvider(DefaultProviderCommands),
}

#[derive(Debug, Subcommand)]
pub enum DefaultProviderCommands {
    #[command(about = "Set the default provider (must exist in the services provider registry)")]
    Set {
        #[arg(
            long,
            help = "Provider name declared in the services provider registry"
        )]
        provider: String,
    },

    #[command(about = "Clear the default provider")]
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum RouteCommands {
    #[command(about = "Add or replace a route (upsert by model pattern)")]
    Add(RouteAddArgs),

    #[command(about = "Remove a route by model pattern")]
    Remove {
        #[arg(long, help = "Model pattern to remove (e.g. claude-*)")]
        model_pattern: String,
    },

    #[command(about = "List configured routes")]
    List,
}

#[derive(Debug, Clone, Args)]
pub struct RouteAddArgs {
    #[arg(long, help = "Model pattern (e.g. claude-*)")]
    pub model_pattern: String,

    #[arg(
        long,
        help = "Provider name (must exist in the services provider registry)"
    )]
    pub provider: String,

    #[arg(long, help = "Upstream model name the provider expects (optional)")]
    pub upstream_model: Option<String>,
}

pub async fn execute(command: &GatewayCommands, config: &CliConfig) -> Result<()> {
    match command {
        GatewayCommands::Route(RouteCommands::List) => list_routes(config),
        GatewayCommands::Enable => apply(config, |file| set_enabled(file, true)).await,
        GatewayCommands::Disable => apply(config, |file| set_enabled(file, false)).await,
        GatewayCommands::Route(RouteCommands::Add(args)) => {
            apply(config, |file| add_route(file, args)).await
        },
        GatewayCommands::Route(RouteCommands::Remove { model_pattern }) => {
            apply(config, |file| remove_route(file, model_pattern)).await
        },
        GatewayCommands::DefaultProvider(DefaultProviderCommands::Set { provider }) => {
            apply(config, |file| set_default_provider(file, provider)).await
        },
        GatewayCommands::DefaultProvider(DefaultProviderCommands::Clear) => {
            apply(config, clear_default_provider).await
        },
    }
}

async fn apply(
    config: &CliConfig,
    mutate: impl FnOnce(&mut GatewayFile) -> Result<String>,
) -> Result<()> {
    let mut file = load_gateway_file()?;
    let message = mutate(&mut file.content)?;

    let registry = &booted_services()?.providers;
    validate_gateway(&file.content, registry)?;
    save_file(&file, gateway_relative())?;
    let source = file.path.display().to_string();
    let outcome =
        super::reconcile::reconcile_authz(file.content.gateway.as_ref(), registry, &source).await;

    render_result(
        &CommandOutput::card_value(
            "Gateway Updated",
            &ConfigMutationOutput {
                field: "gateway".to_owned(),
                message: super::reconcile::append_reconcile_notice(message, &outcome),
            },
        ),
        config,
    );
    Ok(())
}

pub fn spec_mut(file: &mut GatewayFile) -> Result<&mut GatewayConfigSpec> {
    file.gateway
        .get_or_insert_with(|| GatewayState::Spec(GatewayConfigSpec::default()))
        .as_spec_mut()
        .ok_or_else(|| anyhow!("gateway is in a resolved state and cannot be edited"))
}

pub fn set_enabled(file: &mut GatewayFile, enabled: bool) -> Result<String> {
    spec_mut(file)?.enabled = enabled;
    Ok(format!("Gateway enabled = {}", enabled))
}

pub fn add_route(file: &mut GatewayFile, args: &RouteAddArgs) -> Result<String> {
    let mut route = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: args.model_pattern.clone(),
        provider: ProviderId::new(&args.provider),
        upstream_model: args.upstream_model.clone(),
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
    };
    route.ensure_id();
    let spec = spec_mut(file)?;
    spec.routes
        .retain(|r| r.model_pattern != args.model_pattern);
    spec.routes.push(route);
    Ok(format!(
        "Route {} -> {} added",
        args.model_pattern, args.provider
    ))
}

pub fn set_default_provider(file: &mut GatewayFile, provider: &str) -> Result<String> {
    spec_mut(file)?.default_provider = Some(ProviderId::new(provider));
    Ok(format!("Gateway default provider set to {}", provider))
}

pub fn clear_default_provider(file: &mut GatewayFile) -> Result<String> {
    spec_mut(file)?.default_provider = None;
    Ok("Gateway default provider cleared".to_owned())
}

pub fn remove_route(file: &mut GatewayFile, model_pattern: &str) -> Result<String> {
    let spec = spec_mut(file)?;
    let before = spec.routes.len();
    spec.routes.retain(|r| r.model_pattern != model_pattern);
    if spec.routes.len() == before {
        bail!("No route found for model pattern {}", model_pattern);
    }
    Ok(format!("Route {} removed", model_pattern))
}

pub fn validate_gateway(file: &GatewayFile, registry: &ProviderRegistry) -> Result<()> {
    let Some(state) = &file.gateway else {
        return Ok(());
    };
    let resolved = state.clone().into_spec().resolve();
    resolved
        .validate(registry)
        .map_err(|e| anyhow!("gateway validation failed: {e}"))
}

fn list_routes(config: &CliConfig) -> Result<()> {
    let items: Vec<ListItem> = booted_services()?
        .gateway_config()
        .map(|gateway| gateway.routes.clone())
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let route = format!("{} -> {}", r.model_pattern, r.provider.as_str());
            ListItem::new(route, String::new(), String::new())
        })
        .collect();
    render_result(
        &CommandOutput::list(items).with_title("Gateway Routes"),
        config,
    );
    Ok(())
}
