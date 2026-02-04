pub mod auth;
pub mod db;
mod deploy;
pub mod dockerfile;
mod domain;
mod init;
pub mod profile;
mod restart;
mod secrets;
mod status;
pub mod sync;
pub mod templates;
pub mod tenant;
pub mod types;

pub use systemprompt_cloud::{Environment, OAuthProvider};

use crate::cli_settings::CliConfig;
use crate::descriptor::{CommandDescriptor, DescribeCommand};
use anyhow::Result;
use clap::Subcommand;

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

    #[command(about = "Deploy to systemprompt.io Cloud")]
    Deploy {
        #[arg(long)]
        skip_push: bool,

        #[arg(long, short = 'p', help = "Profile name to deploy")]
        profile: Option<String>,

        #[arg(
            long,
            help = "Skip pre-deploy sync from cloud (WARNING: may lose runtime files)"
        )]
        no_sync: bool,

        #[arg(short = 'y', long, help = "Skip confirmation prompts")]
        yes: bool,

        #[arg(long, help = "Preview sync without deploying")]
        dry_run: bool,
    },

    #[command(about = "Check cloud deployment status")]
    Status,

    #[command(about = "Restart tenant machine")]
    Restart {
        #[arg(long)]
        tenant: Option<String>,

        #[arg(short = 'y', long, help = "Skip confirmation prompts")]
        yes: bool,
    },

    #[command(
        subcommand_required = false,
        about = "Sync between local and cloud environments"
    )]
    Sync {
        #[command(subcommand)]
        command: Option<sync::SyncCommands>,
    },

    #[command(subcommand, about = "Manage secrets for cloud tenant")]
    Secrets(secrets::SecretsCommands),

    #[command(about = "Generate Dockerfile based on discovered extensions")]
    Dockerfile,

    #[command(subcommand, about = "Cloud database operations")]
    Db(db::CloudDbCommands),

    #[command(subcommand, about = "Manage custom domain and TLS certificates")]
    Domain(domain::DomainCommands),
}

impl DescribeCommand for CloudCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Sync {
                command: Some(sync::SyncCommands::Local(_)),
            } => CommandDescriptor::PROFILE_SECRETS_AND_PATHS,
            Self::Deploy { .. } => CommandDescriptor::PROFILE_AND_SECRETS,
            Self::Sync { command: Some(_) } | Self::Secrets { .. } => {
                CommandDescriptor::PROFILE_AND_SECRETS
            },
            Self::Status | Self::Restart { .. } | Self::Domain { .. } => {
                CommandDescriptor::PROFILE_ONLY
            },
            _ => CommandDescriptor::NONE,
        }
    }
}

impl CloudCommands {
    pub const fn requires_profile(&self) -> bool {
        matches!(
            self,
            Self::Sync { command: Some(_) }
                | Self::Status
                | Self::Restart { .. }
                | Self::Secrets { .. }
                | Self::Domain { .. }
        )
    }

    pub const fn requires_secrets(&self) -> bool {
        matches!(self, Self::Sync { command: Some(_) } | Self::Secrets { .. })
    }
}

pub async fn execute(cmd: CloudCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        CloudCommands::Auth(cmd) => auth::execute(cmd, config).await,
        CloudCommands::Init { force } => init::execute(force, config),
        CloudCommands::Tenant { command } => tenant::execute(command, config).await,
        CloudCommands::Profile { command } => profile::execute(command, config).await,
        CloudCommands::Deploy {
            skip_push,
            profile,
            no_sync,
            yes,
            dry_run,
        } => {
            deploy::execute(
                deploy::DeployArgs {
                    skip_push,
                    profile_name: profile,
                    no_sync,
                    yes,
                    dry_run,
                },
                config,
            )
            .await
        },
        CloudCommands::Status => {
            let result = status::execute(config).await?;
            crate::shared::render_result(&result);
            Ok(())
        },
        CloudCommands::Restart { tenant, yes } => {
            let result = restart::execute(tenant, yes, config).await?;
            crate::shared::render_result(&result);
            Ok(())
        },
        CloudCommands::Sync { command } => sync::execute(command, config).await,
        CloudCommands::Secrets(cmd) => secrets::execute(cmd, config).await,
        CloudCommands::Dockerfile => execute_dockerfile(config),
        CloudCommands::Db(cmd) => db::execute(cmd, config).await,
        CloudCommands::Domain(cmd) => domain::execute(cmd, config).await,
    }
}

fn execute_dockerfile(config: &CliConfig) -> Result<()> {
    use crate::shared::project::ProjectRoot;
    use types::DockerfileOutput;

    let project = ProjectRoot::discover().map_err(|e| anyhow::anyhow!("{}", e))?;
    let content = dockerfile::generate_dockerfile_content(project.as_path());

    let output = DockerfileOutput {
        content: content.clone(),
    };

    if config.is_json_output() {
        crate::shared::render_result(
            &crate::shared::CommandResult::copy_paste(output).with_title("Dockerfile"),
        );
    } else {
        systemprompt_logging::CliService::info(&content);
    }

    Ok(())
}
