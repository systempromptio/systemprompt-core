//! `admin config` command tree: inspect and edit profile/service configuration.
//!
//! [`ConfigCommands`] dispatches to the show, list, and validate handlers plus
//! the rate-limit, server, runtime, security and paths sub-trees, the model
//! catalog editors (credential, provider, endpoint, model, deployment,
//! gateway), `migrate`, the one reader of the pre-0.73 shape, and `apply`, the
//! deploy pipeline's explicit code-to-database step.
//! Each sub-module owns its clap surface; the group returns the output the
//! kernel renders.
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
pub mod git_credentials;
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
mod validate_jobs;
pub mod validate_services;

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Subcommand;

use systemprompt_extension::cli::{CommandError, CommandOutput};
use systemprompt_loader::ServicesState;
use systemprompt_models::artifacts::NoticeLine;
use serde::Serialize;

use crate::descriptor::{CommandDescriptor, DescribeCommand};

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

impl DescribeCommand for ConfigCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Secret(secret::SecretCommands::MigrateGitCredentials(_)) => {
                CommandDescriptor::FULL
            },
            Self::Secret(_) => CommandDescriptor::PROFILE_ONLY,
            Self::Validate(_) => CommandDescriptor::PROFILE_AND_SECRETS,
            Self::Migrate(args) if args.profiles.is_some() => CommandDescriptor::NONE,
            Self::Migrate(args) if !args.data => CommandDescriptor::PROFILE_AND_SECRETS,
            Self::Catalog(_) => CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_model_discovery(),
            _ => CommandDescriptor::PROFILE_SECRETS_AND_PATHS,
        }
    }
}

pub async fn execute(command: ConfigCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match command {
        ConfigCommands::Show => show::execute(host),
        ConfigCommands::List(args) => list::execute(args, host).await,
        ConfigCommands::Validate(args) => {
            validate_services::prepare(&args, host).await?;
            let (result, valid) = validate::execute(&args, host).await?;
            if !valid {
                return Err(CommandError::new(anyhow::anyhow!("Config validation failed"))
                    .with_output(result)
                    .into());
            }
            Ok(result)
        },
        ConfigCommands::RateLimits(cmd) => rate_limits::execute(cmd, host.prompter(), host),
        ConfigCommands::Server(ref cmd) => server::execute(cmd, host),
        ConfigCommands::Runtime(cmd) => runtime::execute(cmd, host),
        ConfigCommands::Security(ref cmd) => security::execute(cmd, host),
        ConfigCommands::Services(ref cmd) => services::execute(cmd, host),
        ConfigCommands::Paths(cmd) => paths::execute(cmd, host),
        ConfigCommands::Credential(ref cmd) => credential::execute(cmd, host).await,
        ConfigCommands::Provider(ref cmd) => provider::execute(cmd, host).await,
        ConfigCommands::Endpoint(ref cmd) => endpoint::execute(cmd, host).await,
        ConfigCommands::Model(ref cmd) => model::execute(cmd, host).await,
        ConfigCommands::Deployment(ref cmd) => deployment::execute(cmd, host).await,
        ConfigCommands::Gateway(ref cmd) => gateway::execute(cmd, host).await,
        ConfigCommands::Governance(ref cmd) => governance::execute(cmd, host),
        ConfigCommands::Catalog(ref cmd) => {
            let services = host.resolved().await?.optional::<ServicesState>();
            catalog::execute(cmd, services.as_ref())
        },
        ConfigCommands::Migrate(ref args) => migrate::execute(args, host).await,
        ConfigCommands::Secret(ref cmd) => secret::execute(cmd, host).await,
        ConfigCommands::Planes(cmd) => planes::execute(cmd, host).await,
        ConfigCommands::Apply(ref args) => apply::execute(args, host).await,
    }
}

pub(crate) fn restart_note() -> NoticeLine {
    NoticeLine::new("warning", "Restart services for changes to take effect")
}

pub(crate) fn changes_output<T: Serialize>(title: &str, changes: &[T]) -> CommandOutput {
    CommandOutput::table_of(vec!["field", "old_value", "new_value", "message"], changes)
        .with_title(title)
}
