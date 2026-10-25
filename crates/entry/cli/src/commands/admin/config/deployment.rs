//! `admin config deployment` — which endpoints serve a declared model, in
//! failover order (`services/ai/deployments.yaml`).
//!
//! `set` replaces the model's whole chain; the model itself is unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::{ModelSpec, ProviderCatalogService};
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::DeploymentTarget;

use super::catalog_edit::{apply, parse_endpoints, read_active, render_list};
use crate::context::CommandContext;

#[derive(Debug, Subcommand)]
pub enum DeploymentCommands {
    #[command(about = "List every model's deployment chain")]
    List,
    #[command(about = "Replace a model's deployment chain")]
    Set(DeploymentSetArgs),
}

#[derive(Debug, Clone, Args)]
pub struct DeploymentSetArgs {
    #[arg(long, value_parser = crate::shared::parse_model_id)]
    pub model: ModelId,
    #[arg(
        long = "endpoint",
        help = "provider/endpoint, in failover order (repeatable)"
    )]
    pub endpoints: Vec<String>,
}

pub async fn execute(command: &DeploymentCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    match command {
        DeploymentCommands::List => {
            let document = read_active(ctx)?;
            let rows = document
                .deployments
                .iter()
                .map(|(model, targets)| {
                    let chain: Vec<String> =
                        targets.iter().map(|t| t.endpoint().to_string()).collect();
                    format!("{model}: {}", chain.join(" > "))
                })
                .collect();
            render_list(config, "Deployments", rows);
            Ok(())
        },
        DeploymentCommands::Set(args) => {
            let deployments: Vec<DeploymentTarget> = parse_endpoints(&args.endpoints)?
                .into_iter()
                .map(DeploymentTarget::Endpoint)
                .collect();
            apply(ctx, "Deployments Updated", |document, context| {
                if document.models.get(args.model.as_str()).is_none() {
                    bail!(
                        "model '{}' is not declared; add it with `admin config model add`",
                        args.model
                    );
                }
                ProviderCatalogService::upsert_model(
                    document,
                    context,
                    ModelSpec {
                        id: args.model.clone(),
                        upstream_model: None,
                        display_name: None,
                        clients: None,
                        deployments,
                        pricing: None,
                    },
                )?;
                Ok(format!("Deployments of {} replaced", args.model))
            })
            .await
        },
    }
}
