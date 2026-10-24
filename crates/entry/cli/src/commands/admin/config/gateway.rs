//! `admin config gateway` — edit `services/ai/gateway.yaml`: enable state,
//! the one `default_model`, and the policy routes.
//!
//! Routing for the common case is the deployments file; a route here is
//! policy only (a scope or request predicate, a deployment pin, a strategy or
//! an alias) and has a declared id. Every edit re-resolves the whole catalog,
//! so a default model or route naming something the catalog does not declare
//! fails at the edit rather than at the next boot. The profile is never
//! touched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::CatalogDocument;
use systemprompt_manifest::services::gateway::{ModelGlobs, RouteAlias, RouteDeployment};
use systemprompt_manifest::services::{GatewayConfigSpec, GatewayRoute};

use super::catalog_edit::{apply, parse_endpoints, read_active, render_list};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum GatewayCommands {
    #[command(about = "Enable the gateway")]
    Enable,

    #[command(about = "Disable the gateway")]
    Disable,

    #[command(subcommand, about = "Manage gateway policy routes")]
    Route(RouteCommands),

    #[command(subcommand, about = "Manage the one default model")]
    DefaultModel(DefaultModelCommands),
}

#[derive(Debug, Subcommand)]
pub enum DefaultModelCommands {
    #[command(about = "Set gateway.default_model (a catalog model id, no `[1m]`)")]
    Set {
        #[arg(long, value_parser = crate::shared::parse_model_id)]
        model: ModelId,
    },

    #[command(about = "Clear gateway.default_model")]
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum RouteCommands {
    #[command(about = "Add or replace a policy route (upsert by id)")]
    Add(RouteAddArgs),

    #[command(about = "Remove a policy route by id")]
    Remove {
        #[arg(long)]
        id: String,
    },

    #[command(about = "List policy routes")]
    List,
}

#[derive(Debug, Clone, Args)]
pub struct RouteAddArgs {
    #[arg(long, help = "Declared, stable route id")]
    pub id: String,

    #[arg(long, help = "Audit label (defaults to the id)")]
    pub label: Option<String>,

    #[arg(long = "models", help = "Model glob the route applies to (repeatable)")]
    pub models: Vec<String>,

    #[arg(
        long = "deployment",
        help = "Pin the chain to these provider/endpoint deployments (repeatable)"
    )]
    pub deployments: Vec<String>,

    #[arg(
        long,
        help = "Alias as REQUESTED=MODEL: the requested id resolves to MODEL"
    )]
    pub alias: Option<String>,
}

pub async fn execute(command: &GatewayCommands, config: &CliConfig) -> Result<()> {
    match command {
        GatewayCommands::Route(RouteCommands::List) => list_routes(config),
        GatewayCommands::Enable => edit(config, |spec| Ok(set_enabled(spec, true))).await,
        GatewayCommands::Disable => edit(config, |spec| Ok(set_enabled(spec, false))).await,
        GatewayCommands::Route(RouteCommands::Add(args)) => {
            edit(config, |spec| add_route(spec, args)).await
        },
        GatewayCommands::Route(RouteCommands::Remove { id }) => {
            edit(config, |spec| remove_route(spec, id)).await
        },
        GatewayCommands::DefaultModel(DefaultModelCommands::Set { model }) => {
            edit(config, |spec| {
                spec.default_model = Some(model.clone());
                Ok(format!("Gateway default model set to {model}"))
            })
            .await
        },
        GatewayCommands::DefaultModel(DefaultModelCommands::Clear) => {
            edit(config, |spec| {
                spec.default_model = None;
                Ok("Gateway default model cleared".to_owned())
            })
            .await
        },
    }
}

async fn edit(
    config: &CliConfig,
    mutate: impl FnOnce(&mut GatewayConfigSpec) -> Result<String>,
) -> Result<()> {
    apply(config, "Gateway Updated", |document, context| {
        let message = mutate(spec_mut(document))?;
        document
            .resolve(context)
            .map_err(|e| anyhow!("gateway validation failed: {e}"))?;
        Ok(message)
    })
    .await
}

pub fn spec_mut(document: &mut CatalogDocument) -> &mut GatewayConfigSpec {
    document
        .gateway
        .get_or_insert_with(GatewayConfigSpec::default)
}

pub fn set_enabled(spec: &mut GatewayConfigSpec, enabled: bool) -> String {
    spec.enabled = enabled;
    format!("Gateway enabled = {enabled}")
}

pub fn add_route(spec: &mut GatewayConfigSpec, args: &RouteAddArgs) -> Result<String> {
    let mut route = GatewayRoute::new(&args.id);
    route.label.clone_from(&args.label);
    route.models = ModelGlobs(args.models.clone());
    route.deployments = parse_endpoints(&args.deployments)?
        .into_iter()
        .map(RouteDeployment::Endpoint)
        .collect();
    if let Some(raw) = args.alias.as_deref() {
        let Some((requested, model)) = raw.split_once('=') else {
            bail!("invalid --alias '{raw}'; expected REQUESTED=MODEL");
        };
        route.alias = Some(RouteAlias {
            requested: ModelId::new(requested.trim()),
            model: ModelId::new(model.trim()),
        });
    }
    spec.routes.retain(|r| r.id != route.id);
    spec.routes.push(route);
    Ok(format!("Route {} saved", args.id))
}

pub fn remove_route(spec: &mut GatewayConfigSpec, id: &str) -> Result<String> {
    let before = spec.routes.len();
    spec.routes.retain(|r| r.id.as_str() != id);
    if spec.routes.len() == before {
        bail!("No route with id {id}");
    }
    Ok(format!("Route {id} removed"))
}

fn list_routes(config: &CliConfig) -> Result<()> {
    let document = read_active()?;
    let rows = document
        .gateway
        .map(|gateway| gateway.routes)
        .unwrap_or_default()
        .iter()
        .map(|r| format!("{} ({}) models {:?}", r.id, r.audit_label(), r.models.0))
        .collect();
    render_list(config, "Gateway Routes", rows);
    Ok(())
}
