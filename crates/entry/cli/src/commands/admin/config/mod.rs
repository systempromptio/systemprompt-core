//! `admin config` command tree: inspect and edit profile/service configuration.
//!
//! [`ConfigCommands`] dispatches to the show, list, and validate handlers plus
//! the rate-limit, server, runtime, security and paths sub-trees, the model
//! catalog editors (credential, provider, endpoint, model, deployment,
//! gateway), `migrate`, the one reader of the pre-0.73 shape, and `apply`, the
//! deploy pipeline's explicit code-to-database step.
//! Each sub-module owns its clap surface and renders results through
//! [`crate::shared::render_result`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod apply;
pub mod catalog;
pub mod catalog_discovery;
pub mod catalog_edit;
pub mod config_section;
pub mod credential;
pub mod deployment;
pub mod endpoint;
pub mod gateway;
pub mod governance;
pub mod list;
pub mod migrate;
pub mod model;
pub mod paths;
pub mod plane_validate;
pub mod planes;
pub mod profile_io;
pub mod provider;
pub mod rate_limit_types;
pub mod rate_limits;
pub mod reconcile;
pub mod runtime;
pub mod secret;
pub mod secret_check;
pub mod security;
pub mod server;
pub mod services;
pub mod services_io;
pub mod show;
pub mod types;
pub mod validate;

use anyhow::{Result, bail};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(about = "Show configuration overview")]
    Show,

    #[command(about = "List all configuration files")]
    List(list::ListArgs),

    #[command(about = "Validate configuration files")]
    Validate(validate::ValidateArgs),

    #[command(subcommand, about = "Rate limit configuration")]
    RateLimits(rate_limits::RateLimitsCommands),

    #[command(subcommand, about = "Server configuration")]
    Server(server::ServerCommands),

    #[command(subcommand, about = "Runtime configuration")]
    Runtime(runtime::RuntimeCommands),

    #[command(subcommand, about = "Security configuration")]
    Security(security::SecurityCommands),

    #[command(subcommand, about = "Services configuration")]
    Services(services::ServicesCommands),

    #[command(subcommand, about = "Paths configuration")]
    Paths(paths::PathsCommands),

    #[command(
        subcommand,
        about = "Catalog credentials (services/ai/credentials.yaml): who authenticates and pays"
    )]
    Credential(credential::CredentialCommands),

    #[command(subcommand, about = "Catalog providers (services/ai/providers.yaml)")]
    Provider(provider::ProviderCommands),

    #[command(
        subcommand,
        about = "Catalog endpoints: one wire plus one URL under a provider"
    )]
    Endpoint(endpoint::EndpointCommands),

    #[command(subcommand, about = "Catalog models (services/ai/models.yaml)")]
    Model(model::ModelCommands),

    #[command(
        subcommand,
        about = "Catalog deployments (services/ai/deployments.yaml): which model is served where"
    )]
    Deployment(deployment::DeploymentCommands),

    #[command(
        subcommand,
        about = "Gateway configuration (default model, policy routes)"
    )]
    Gateway(gateway::GatewayCommands),

    #[command(subcommand, about = "Governance authorization hook configuration")]
    Governance(governance::GovernanceCommands),

    #[command(subcommand, about = "Model catalog views (Vertex discovery)")]
    Catalog(catalog::CatalogCommands),

    #[command(about = "Rewrite a pre-0.73 services tree to the catalog shape (and move its data)")]
    Migrate(migrate::MigrateArgs),

    #[command(subcommand, about = "Profile secrets")]
    Secret(secret::SecretCommands),

    #[command(
        subcommand,
        about = "Declared config planes: list, diff, export and apply (file vs database authority)"
    )]
    Planes(planes::PlanesCommands),

    #[command(
        about = "Apply declared config to the database: three-way merge, conflicts left for \
                 /admin/sync (run by the deploy pipeline)"
    )]
    Apply(apply::ApplyArgs),
}

pub async fn execute(command: ConfigCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        ConfigCommands::Show => {
            let result = show::execute(&ctx.cli)?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        ConfigCommands::List(args) => {
            let result = list::execute(args, &ctx.cli);
            render_result(&result, &ctx.cli);
            Ok(())
        },
        ConfigCommands::Validate(args) => {
            let (result, valid) = validate::execute(&args, &ctx.cli)?;
            render_result(&result, &ctx.cli);
            if !valid {
                bail!("Config validation failed");
            }
            Ok(())
        },
        ConfigCommands::RateLimits(cmd) => rate_limits::execute(cmd, ctx.prompter(), &ctx.cli),
        ConfigCommands::Server(ref cmd) => server::execute(cmd, &ctx.cli),
        ConfigCommands::Runtime(cmd) => runtime::execute(cmd, &ctx.cli),
        ConfigCommands::Security(ref cmd) => security::execute(cmd, &ctx.cli),
        ConfigCommands::Services(ref cmd) => services::execute(cmd, &ctx.cli),
        ConfigCommands::Paths(cmd) => paths::execute(cmd, &ctx.cli),
        ConfigCommands::Credential(ref cmd) => credential::execute(cmd, &ctx.cli).await,
        ConfigCommands::Provider(ref cmd) => provider::execute(cmd, &ctx.cli).await,
        ConfigCommands::Endpoint(ref cmd) => endpoint::execute(cmd, &ctx.cli).await,
        ConfigCommands::Model(ref cmd) => model::execute(cmd, &ctx.cli).await,
        ConfigCommands::Deployment(ref cmd) => deployment::execute(cmd, &ctx.cli).await,
        ConfigCommands::Gateway(ref cmd) => gateway::execute(cmd, &ctx.cli).await,
        ConfigCommands::Governance(ref cmd) => governance::execute(cmd, &ctx.cli),
        ConfigCommands::Catalog(ref cmd) => catalog::execute(cmd, &ctx.cli),
        ConfigCommands::Migrate(ref args) => migrate::execute(args, &ctx.cli).await,
        ConfigCommands::Secret(ref cmd) => secret::execute(cmd, &ctx.cli).await,
        ConfigCommands::Planes(cmd) => planes::execute(cmd, &ctx.cli).await,
        ConfigCommands::Apply(ref args) => apply::execute(args, &ctx.cli).await,
    }
}
