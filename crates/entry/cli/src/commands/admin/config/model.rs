//! `admin config model` — what a model is, stated once
//! (`services/ai/models.yaml`), and where it is first deployed.
//!
//! `add` needs at least one `--deployment provider/endpoint`: a model with no
//! deployment cannot resolve. Unset options keep their stored value, so a
//! re-add never drops a model's pricing, limits or capabilities.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_config::{ModelSpec, ProviderCatalogService};
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::DeploymentTarget;

use super::catalog_edit::{apply, parse_clients, parse_endpoints, read_active, render_list};
use crate::context::CommandContext;

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    #[command(about = "List declared models with their deployments")]
    List,
    #[command(about = "Add or update a model and its deployments")]
    Add(ModelAddArgs),
    #[command(about = "Remove a model and its deployments")]
    Remove {
        #[arg(long, value_parser = crate::shared::parse_model_id)]
        id: ModelId,
    },
}

#[derive(Debug, Clone, Args)]
pub struct ModelAddArgs {
    #[arg(long, value_parser = crate::shared::parse_model_id)]
    pub id: ModelId,
    #[arg(
        long = "deployment",
        help = "provider/endpoint serving the model, in failover order (repeatable)"
    )]
    pub deployments: Vec<String>,
    #[arg(
        long,
        help = "Name the upstream knows the model by (defaults to the id)"
    )]
    pub upstream_model: Option<String>,
    #[arg(long)]
    pub display_name: Option<String>,
    #[arg(
        long = "clients",
        help = "Client families it is advertised to: claude,openai,gemini, or none"
    )]
    pub clients: Option<Vec<String>>,
}

pub async fn execute(command: &ModelCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    match command {
        ModelCommands::List => {
            let document = read_active(ctx)?;
            let rows = document
                .models
                .iter()
                .map(|(id, model)| {
                    let deployed: Vec<String> = document
                        .deployments
                        .get(id.as_str())
                        .unwrap_or_default()
                        .iter()
                        .map(|t| t.endpoint().to_string())
                        .collect();
                    let clients: Vec<&str> = model.clients.iter().map(|c| c.as_tag()).collect();
                    format!(
                        "{id} -> {} [clients: {}]",
                        deployed.join(" > "),
                        clients.join(",")
                    )
                })
                .collect();
            render_list(config, "Models", rows);
            Ok(())
        },
        ModelCommands::Add(args) => {
            let deployments = parse_endpoints(&args.deployments)?
                .into_iter()
                .map(DeploymentTarget::Endpoint)
                .collect();
            let clients = args.clients.as_deref().map(parse_clients).transpose()?;
            let spec = ModelSpec {
                id: args.id.clone(),
                upstream_model: args.upstream_model.clone(),
                display_name: args.display_name.clone(),
                clients,
                deployments,
                pricing: None,
            };
            apply(ctx, "Model Updated", |document, context| {
                ProviderCatalogService::upsert_model(document, context, spec)?;
                Ok(format!("Model {} saved", args.id))
            })
            .await
        },
        ModelCommands::Remove { id } => {
            apply(ctx, "Model Removed", |document, context| {
                ProviderCatalogService::remove_model(document, context, id)?;
                Ok(format!("Model {id} removed"))
            })
            .await
        },
    }
}
