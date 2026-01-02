pub mod auth;
pub mod checkout;
mod deploy;
pub mod deploy_select;
pub mod dockerfile;
mod init;
mod init_templates;
mod logs;
mod oauth;
pub mod profile;
mod restart;
mod secrets;
mod status;
pub mod sync;
pub mod tenant;
mod tenant_ops;

pub use systemprompt_cloud::{Environment, OAuthProvider};

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
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

    #[command(about = "Deploy to SystemPrompt Cloud")]
    Deploy {
        #[arg(long)]
        skip_push: bool,

        #[arg(long, short = 'p', help = "Profile name to deploy")]
        profile: Option<String>,
    },

    #[command(about = "Check cloud deployment status")]
    Status,

    #[command(about = "View tenant logs")]
    Logs {
        #[arg(long)]
        tenant: Option<String>,

        #[arg(long, short = 'n', default_value = "100")]
        lines: u32,
    },

    #[command(about = "Restart tenant machine")]
    Restart {
        #[arg(long)]
        tenant: Option<String>,
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
}

impl CloudCommands {
    pub fn requires_profile(&self) -> bool {
        match self {
            Self::Deploy { .. } => false,
            Self::Sync { command: None } => false,
            Self::Sync { command: Some(_) } => true,
            Self::Status | Self::Logs { .. } | Self::Restart { .. } | Self::Secrets { .. } => true,
            _ => false,
        }
    }
}

pub async fn execute(cmd: CloudCommands) -> Result<()> {
    match cmd {
        CloudCommands::Auth(cmd) => auth::execute(cmd).await,
        CloudCommands::Init { force } => init::execute(force).await,
        CloudCommands::Tenant { command } => tenant::execute(command).await,
        CloudCommands::Profile { command } => profile::execute(command).await,
        CloudCommands::Deploy { skip_push, profile } => deploy::execute(skip_push, profile).await,
        CloudCommands::Status => status::execute().await,
        CloudCommands::Logs { tenant, lines } => logs::execute(tenant, lines).await,
        CloudCommands::Restart { tenant } => restart::execute(tenant).await,
        CloudCommands::Sync { command } => sync::execute(command).await,
        CloudCommands::Secrets(cmd) => secrets::execute(cmd).await,
        CloudCommands::Dockerfile => execute_dockerfile().await,
    }
}

async fn execute_dockerfile() -> Result<()> {
    use crate::common::project::ProjectRoot;

    let project = ProjectRoot::discover().map_err(|e| anyhow::anyhow!("{}", e))?;
    dockerfile::print_dockerfile_suggestion(project.as_path());
    Ok(())
}
