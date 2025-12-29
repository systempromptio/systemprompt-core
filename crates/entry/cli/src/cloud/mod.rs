pub mod auth;
pub mod checkout;
mod deploy;
mod init;
mod init_templates;
mod oauth;
pub mod profile;
mod status;
pub mod sync;
pub mod tenant;
mod tenant_ops;

pub use systemprompt_cloud::{Environment, OAuthProvider};

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum CloudCommands {
    /// Authentication (login, logout, whoami)
    #[command(subcommand)]
    Auth(auth::AuthCommands),

    /// Initialize project structure
    Init {
        /// Overwrite existing files
        #[arg(long)]
        force: bool,
    },

    /// Manage tenants (local or cloud)
    #[command(subcommand_required = false)]
    Tenant {
        #[command(subcommand)]
        command: Option<tenant::TenantCommands>,
    },

    /// Manage profiles (create, list, show, delete)
    #[command(subcommand_required = false)]
    Profile {
        #[command(subcommand)]
        command: Option<profile::ProfileCommands>,
    },

    /// Deploy to SystemPrompt Cloud
    Deploy {
        /// Skip Docker push step
        #[arg(long)]
        skip_push: bool,

        /// Custom image tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Check cloud deployment status
    Status,

    /// Sync between local and cloud environments
    #[command(subcommand)]
    Sync(sync::SyncCommands),
}

impl CloudCommands {
    pub fn requires_profile(&self) -> bool {
        matches!(self, Self::Deploy { .. } | Self::Status | Self::Sync { .. })
    }
}

pub async fn execute(cmd: CloudCommands) -> Result<()> {
    match cmd {
        CloudCommands::Auth(cmd) => auth::execute(cmd).await,
        CloudCommands::Init { force } => init::execute(force).await,
        CloudCommands::Tenant { command } => tenant::execute(command).await,
        CloudCommands::Profile { command } => profile::execute(command).await,
        CloudCommands::Deploy { skip_push, tag } => deploy::execute(skip_push, tag).await,
        CloudCommands::Status => status::execute().await,
        CloudCommands::Sync(cmd) => sync::execute(cmd).await,
    }
}
