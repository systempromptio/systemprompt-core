//! `cloud` command tree for systemprompt.io Cloud.
//!
//! Routes [`CloudCommands`] to the auth, init, tenant, profile, deploy,
//! backup, status, fleet, dockerfile and sync subcommands, and declares each
//! command's profile/secret requirements via [`DescribeCommand`]. Every
//! descriptor is a cloud-operator descriptor: the runner loads the operator's
//! cloud credentials for it and lets it run on a cloud profile that arrived
//! implicitly. The cloud member declares the group as the top-level `cloud`
//! command with the `CloudOperator` grant, in the `--help` slot after `admin`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admitted_views;
pub mod auth;
pub mod backup;
pub mod deploy;
pub mod doctor;
pub mod fleet;
pub mod init;
pub mod profile;
mod status;
pub mod sync;
pub mod templates;
pub mod tenant;
pub mod types;

pub use admitted_views::AdmittedViews;
pub use systemprompt_cloud::{Environment, OAuthProvider};

use anyhow::Result;
use clap::Subcommand;
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};
use systemprompt_identifiers::ProfileName;

#[derive(Debug, Subcommand)]
pub enum CloudCommands {
    #[command(subcommand, about = "Authentication (login, logout, whoami)")]
    Auth(auth::AuthCommands),

    #[command(about = "Initialize project structure")]
    Init {
        #[arg(long)]
        force: bool,
    },

    #[command(subcommand_required = false, about = "Manage tenants (local or cloud)")]
    Tenant {
        #[command(subcommand)]
        command: Option<tenant::TenantCommands>,
    },

    #[command(subcommand_required = false, about = "Manage profiles")]
    Profile {
        #[command(subcommand)]
        command: Option<profile::ProfileCommands>,
    },

    #[command(about = "Deploy the profile's tenant (systemprompt.io Cloud or GitOps)")]
    Deploy {
        #[arg(long)]
        skip_push: bool,

        #[arg(
            long,
            short = 'p',
            value_parser = systemprompt_extension::cli::parsers::parse_profile_name_arg,
            help = "Profile name to deploy"
        )]
        profile: Option<String>,

        #[arg(long, help = "Run the pre-deploy preflight only, without deploying")]
        check: bool,

        #[arg(
            long,
            help = "After the rollout, wait until the tenant's /readyz reports ready"
        )]
        wait: bool,
    },

    #[command(about = "Download the tenant's runtime services/ tree to a local directory")]
    Backup {
        #[arg(
            long,
            short = 'p',
            value_parser = systemprompt_extension::cli::parsers::parse_profile_name_arg,
            help = "Profile name to back up"
        )]
        profile: Option<String>,

        #[arg(
            long,
            short = 'o',
            help = "Output directory (default: ./systemprompt-backup-<timestamp>)"
        )]
        output: Option<std::path::PathBuf>,

        #[arg(long, help = "List remote files without downloading")]
        list: bool,
    },

    #[command(about = "Run the pre-deploy preflight for a profile without deploying")]
    Doctor {
        #[arg(
            long,
            short = 'p',
            value_parser = systemprompt_extension::cli::parsers::parse_profile_name_arg,
            help = "Profile name to check"
        )]
        profile: Option<String>,

        #[arg(
            long,
            help = "Also run the multi-replica checks: identity fingerprints, instance id, \
                    trusted proxies, write primary, replica lag, readiness"
        )]
        distributed: bool,
    },

    #[command(about = "Check cloud deployment status")]
    Status,

    #[command(
        subcommand,
        about = "Across every tenant: run read-only commands, report versions"
    )]
    Fleet(fleet::FleetCommands),

    #[command(about = "Generate Dockerfile based on discovered extensions")]
    Dockerfile,

    #[command(
        subcommand,
        about = "Database-to-code sync: export console changes as declarations"
    )]
    Sync(sync::SyncCommands),
}

impl DescribeCommand for CloudCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Deploy { .. } => CommandDescriptor::NONE.with_data_impact(DataImpact::Preserving),
            Self::Backup { .. } | Self::Status => CommandDescriptor::PROFILE_ONLY,
            Self::Sync(_) => CommandDescriptor::FULL.with_read_only(),
            _ => CommandDescriptor::NONE,
        }
    }
}

impl CliCommandGroup for CloudCommands {
    const NAME: &'static str = "cloud";
    const ABOUT: &'static str = "Cloud deployment, backup, and setup";
    const DISPLAY_ORDER: usize = 40;

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(CommandError::new)
    }
}

fn profile_name(profile: Option<String>) -> Result<Option<ProfileName>> {
    Ok(profile.map(ProfileName::try_new).transpose()?)
}

pub async fn execute(cmd: CloudCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        CloudCommands::Auth(cmd) => auth::execute(cmd, host).await,
        CloudCommands::Init { force } => {
            init::execute(force, host.output(), host.children())?;
            Ok(CommandOutput::rendered())
        },
        CloudCommands::Tenant { command } => tenant::execute(command, host).await,
        CloudCommands::Profile { command } => profile::execute(command, host).await,
        CloudCommands::Deploy {
            skip_push,
            profile,
            check,
            wait,
        } => {
            deploy::execute(
                deploy::DeployArgs {
                    skip_push,
                    profile_name: profile_name(profile)?,
                    check,
                    wait,
                },
                host.prompter(),
                host.output(),
                host.children(),
                &AdmittedViews::resolve(host).await?,
            )
            .await?;
            Ok(CommandOutput::rendered())
        },
        CloudCommands::Backup {
            profile,
            output,
            list,
        } => {
            backup::execute(
                backup::BackupArgs {
                    profile_name: profile_name(profile)?,
                    output,
                    list,
                },
                host.prompter(),
                host.output(),
            )
            .await?;
            Ok(CommandOutput::rendered())
        },
        CloudCommands::Doctor {
            profile,
            distributed,
        } => {
            doctor::execute(
                profile_name(profile)?,
                distributed,
                host.prompter(),
                host.output(),
                &AdmittedViews::resolve(host).await?,
            )
            .await?;
            Ok(CommandOutput::rendered())
        },
        CloudCommands::Status => {
            let credentials = CredentialsBootstrap::load().await?;
            status::execute(host.output(), host.profile().ok(), credentials.as_ref()).await
        },
        CloudCommands::Fleet(cmd) => fleet::execute(cmd, host).await,
        CloudCommands::Dockerfile => execute_dockerfile(host),
        CloudCommands::Sync(cmd) => sync::execute(cmd, host).await,
    }
}

fn execute_dockerfile(host: &dyn CommandHost) -> Result<CommandOutput> {
    use systemprompt_cloud::deploy::DockerfileBuilder;
    use systemprompt_cloud::paths::ProjectRoot;

    let project = ProjectRoot::discover()?;
    let profile = match host.profile() {
        Ok(profile) => Some(profile),
        Err(error) => {
            tracing::debug!(%error, "no active profile; rendering the default Dockerfile");
            None
        },
    };
    let profile_name = profile
        .map(|p| ProfileName::try_new(p.name.as_str()))
        .transpose()?;
    let image = profile
        .and_then(|p| p.cloud.as_ref())
        .and_then(|cloud| cloud.image.clone())
        .unwrap_or_default();
    let mut builder = DockerfileBuilder::new(project.as_path()).with_image(image);
    if let Some(name) = profile_name.as_ref() {
        builder = builder.with_profile(name);
    }
    let content = builder.build();

    if host.output().is_json_output() {
        return Ok(CommandOutput::copy_paste_titled("Dockerfile", content));
    }
    systemprompt_logging::CliService::output(&content);
    Ok(CommandOutput::rendered())
}
