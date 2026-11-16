//! Session management commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod list;
pub mod login;
pub mod login_helpers;
mod logout;
pub mod show;
mod switch;
pub mod types;

use crate::kernel::KernelHost;
use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_cloud::{CloudError, SessionStore};
use systemprompt_identifiers::ProfileName;
use systemprompt_logging::CliService;

use systemprompt_extension::cli::{CommandDescriptor, DescribeCommand};
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    #[command(about = "Show current session and routing info", alias = "current")]
    Show,

    #[command(about = "Switch to a different profile")]
    Switch {
        #[arg(value_parser = systemprompt_extension::cli::parsers::parse_profile_name)]
        profile_name: ProfileName,
    },

    #[command(about = "List available profiles")]
    List,

    #[command(about = "Create an admin session for CLI access")]
    Login(login::LoginArgs),

    #[command(about = "Remove a session")]
    Logout(logout::LogoutArgs),
}

impl DescribeCommand for SessionCommands {
    fn descriptor(&self) -> CommandDescriptor {
        let descriptor = match self {
            Self::Login(_) => CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_skip_validation(),
            Self::Switch { .. } | Self::Show | Self::List | Self::Logout(_) => {
                CommandDescriptor::NONE
            },
        };
        descriptor.with_cloud_operator()
    }
}

pub async fn execute(cmd: SessionCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        SessionCommands::Show => {
            let result = show::execute(host.output());
            Ok(result)
        },
        SessionCommands::Switch { profile_name } => {
            let result = switch::execute(&profile_name)?;
            Ok(result)
        },
        SessionCommands::List => {
            let result = list::execute(host.output());
            Ok(result)
        },
        SessionCommands::Login(args) => {
            let result = login::execute(args, host).await?;
            Ok(result)
        },
        SessionCommands::Logout(ref args) => {
            let result = logout::execute(args, host.prompter(), host.output())?;
            Ok(result)
        },
    }
}

// Why: `switch` is the documented repair path for a corrupt index — the
// error text sends the operator here — so it alone may start from an empty
// store; every writer that is not a repair surfaces the corruption instead.
pub(super) fn load_or_reset_corrupt(sessions_dir: &Path) -> Result<SessionStore> {
    match SessionStore::load(sessions_dir) {
        Ok(store) => Ok(store.unwrap_or_else(SessionStore::new)),
        Err(CloudError::SessionStoreCorrupted { path, .. }) => {
            CliService::warning(&format!(
                "Session store at {path} is corrupt; starting fresh"
            ));
            Ok(SessionStore::new())
        },
        Err(error) => Err(error.into()),
    }
}

pub(super) fn load_for_display(sessions_dir: &Path) -> SessionStore {
    match SessionStore::load(sessions_dir) {
        Ok(store) => store.unwrap_or_else(SessionStore::new),
        Err(error) => {
            CliService::warning(&format!("Session store unreadable: {error}"));
            SessionStore::new()
        },
    }
}
