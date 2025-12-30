pub mod auth;
pub mod checkout;
mod deploy;
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
    },

    /// Check cloud deployment status
    Status,

    /// View tenant logs
    Logs {
        /// Tenant ID (uses profile tenant if not specified)
        #[arg(long)]
        tenant: Option<String>,

        /// Number of lines to show
        #[arg(long, short = 'n', default_value = "100")]
        lines: u32,
    },

    /// Restart tenant machine
    Restart {
        /// Tenant ID (uses profile tenant if not specified)
        #[arg(long)]
        tenant: Option<String>,
    },

    /// Sync between local and cloud environments
    #[command(subcommand)]
    Sync(sync::SyncCommands),

    /// Manage secrets for cloud tenant
    #[command(subcommand)]
    Secrets(secrets::SecretsCommands),
}

impl CloudCommands {
    pub fn requires_profile(&self) -> bool {
        matches!(
            self,
            Self::Deploy { .. }
                | Self::Status
                | Self::Logs { .. }
                | Self::Restart { .. }
                | Self::Sync { .. }
                | Self::Secrets { .. }
        )
    }
}

pub async fn execute(cmd: CloudCommands) -> Result<()> {
    match cmd {
        CloudCommands::Auth(cmd) => auth::execute(cmd).await,
        CloudCommands::Init { force } => init::execute(force).await,
        CloudCommands::Tenant { command } => tenant::execute(command).await,
        CloudCommands::Profile { command } => profile::execute(command).await,
        CloudCommands::Deploy { skip_push } => deploy::execute(skip_push).await,
        CloudCommands::Status => status::execute().await,
        CloudCommands::Logs { tenant, lines } => logs::execute(tenant, lines).await,
        CloudCommands::Restart { tenant } => restart::execute(tenant).await,
        CloudCommands::Sync(cmd) => sync::execute(cmd).await,
        CloudCommands::Secrets(cmd) => secrets::execute(cmd).await,
    }
}
