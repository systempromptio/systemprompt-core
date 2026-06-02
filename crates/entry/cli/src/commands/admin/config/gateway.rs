//! `admin config gateway` — edit the profile's gateway section: enable state,
//! routing patterns, and the default provider.
//!
//! Every mutation resolves the resulting spec and validates it against the
//! profile's provider registry (`profile.providers`), so a route or
//! default-provider that names a provider absent from the registry fails at the
//! edit rather than at the next boot. The gateway owns no catalog: providers
//! and models live in `profile.providers` (see `admin config catalog`).

use std::collections::HashMap;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::Profile;
use systemprompt_models::profile::{GatewayConfigSpec, GatewayRoute, GatewayState};

use super::profile_io::{load_profile, save_profile};
use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::shared::{CommandResult, render_result};

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
    #[command(about = "Set the default provider (must exist in profile.providers)")]
    Set {
        #[arg(long, help = "Provider name declared in profile.providers")]
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

    #[arg(long, help = "Provider name (must exist in profile.providers)")]
    pub provider: String,

    #[arg(long, help = "Upstream model name the provider expects (optional)")]
    pub upstream_model: Option<String>,
}

pub async fn execute(command: &GatewayCommands, _config: &CliConfig) -> Result<()> {
    if matches!(command, GatewayCommands::Route(RouteCommands::List)) {
        return list_routes();
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let message = match command {
        GatewayCommands::Enable => set_enabled(&mut profile, true)?,
        GatewayCommands::Disable => set_enabled(&mut profile, false)?,
        GatewayCommands::Route(RouteCommands::Add(args)) => add_route(&mut profile, args)?,
        GatewayCommands::Route(RouteCommands::Remove { model_pattern }) => {
            remove_route(&mut profile, model_pattern)?
        },
        GatewayCommands::Route(RouteCommands::List) => unreachable!("handled above"),
        GatewayCommands::DefaultProvider(DefaultProviderCommands::Set { provider }) => {
            set_default_provider(&mut profile, provider)?
        },
        GatewayCommands::DefaultProvider(DefaultProviderCommands::Clear) => {
            clear_default_provider(&mut profile)?
        },
    };

    validate_gateway(&profile)?;
    save_profile(&profile, profile_path)?;
    super::reconcile::reconcile_authz(&profile, profile_path).await;

    render_result(
        &CommandResult::text(ConfigMutationOutput {
            field: "gateway".to_owned(),
            message,
        })
        .with_title("Gateway Updated"),
    );
    Ok(())
}

fn spec_mut(profile: &mut Profile) -> Result<&mut GatewayConfigSpec> {
    profile
        .gateway
        .get_or_insert_with(|| GatewayState::Spec(GatewayConfigSpec::default()))
        .as_spec_mut()
        .ok_or_else(|| anyhow!("gateway is in a resolved state and cannot be edited"))
}

fn set_enabled(profile: &mut Profile, enabled: bool) -> Result<String> {
    spec_mut(profile)?.enabled = enabled;
    Ok(format!("Gateway enabled = {}", enabled))
}

fn add_route(profile: &mut Profile, args: &RouteAddArgs) -> Result<String> {
    let mut route = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: args.model_pattern.clone(),
        provider: ProviderId::new(&args.provider),
        upstream_model: args.upstream_model.clone(),
        extra_headers: HashMap::new(),
        pricing: None,
    };
    route.ensure_id();
    let spec = spec_mut(profile)?;
    spec.routes
        .retain(|r| r.model_pattern != args.model_pattern);
    spec.routes.push(route);
    Ok(format!(
        "Route {} -> {} added",
        args.model_pattern, args.provider
    ))
}

fn set_default_provider(profile: &mut Profile, provider: &str) -> Result<String> {
    spec_mut(profile)?.default_provider = Some(ProviderId::new(provider));
    Ok(format!("Gateway default provider set to {}", provider))
}

fn clear_default_provider(profile: &mut Profile) -> Result<String> {
    spec_mut(profile)?.default_provider = None;
    Ok("Gateway default provider cleared".to_owned())
}

fn remove_route(profile: &mut Profile, model_pattern: &str) -> Result<String> {
    let spec = spec_mut(profile)?;
    let before = spec.routes.len();
    spec.routes.retain(|r| r.model_pattern != model_pattern);
    if spec.routes.len() == before {
        bail!("No route found for model pattern {}", model_pattern);
    }
    Ok(format!("Route {} removed", model_pattern))
}

fn validate_gateway(profile: &Profile) -> Result<()> {
    let Some(state) = &profile.gateway else {
        return Ok(());
    };
    let resolved = state.clone().into_spec().resolve();
    resolved
        .validate(&profile.providers)
        .map_err(|e| anyhow!("gateway validation failed: {e}"))
}

fn list_routes() -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let profile = load_profile(profile_path)?;
    let routes: Vec<String> = profile
        .gateway
        .map(|state| state.into_spec().routes)
        .unwrap_or_default()
        .iter()
        .map(|r| format!("{} -> {}", r.model_pattern, r.provider.as_str()))
        .collect();

    render_result(&CommandResult::list(routes).with_title("Gateway Routes"));
    Ok(())
}
