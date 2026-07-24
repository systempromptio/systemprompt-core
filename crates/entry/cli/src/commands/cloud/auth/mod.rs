//! `cloud auth` subcommands: login, logout, whoami, and admin-user.
//!
//! Dispatches the [`AuthCommands`] enum to the per-command modules that manage
//! the locally stored cloud credentials and authenticated-user state,
//! including projecting the authenticated cloud user as an admin into local
//! profile databases.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admin_user;
mod login;
mod logout;
mod whoami;

use anyhow::anyhow;
use systemprompt_logging::CliService;

pub use login::{build_login_output, complete_login};

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::Result;
use clap::{Args, Subcommand};

use super::Environment;

#[derive(Debug, Subcommand)]
pub enum AuthCommands {
    #[command(about = "Authenticate with systemprompt.io Cloud via OAuth")]
    Login {
        #[arg(value_enum, default_value_t = Environment::default(), hide = true)]
        environment: Environment,
    },

    #[command(about = "Clear saved cloud credentials")]
    Logout(LogoutArgs),

    #[command(
        about = "Show current authenticated user and token status",
        alias = "status"
    )]
    Whoami,

    #[command(about = "Sync cloud user as admin to local profile databases")]
    AdminUser(AdminUserSyncArgs),
}

#[derive(Debug, Args)]
pub struct AdminUserSyncArgs {
    #[arg(short, long, help = "Show detailed discovery information")]
    pub verbose: bool,

    #[arg(long, help = "Specific profile to sync (default: all profiles)")]
    pub profile: Option<String>,

    #[arg(long, help = "Override database URL (requires --profile)")]
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct LogoutArgs {
    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(cmd: AuthCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        AuthCommands::Login { environment } => {
            let result = login::execute(environment, ctx.prompter(), &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AuthCommands::Logout(args) => {
            let result = logout::execute(args, ctx.prompter(), &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AuthCommands::Whoami => {
            let result = whoami::execute(&ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AuthCommands::AdminUser(args) => execute_admin_user_sync(args).await,
    }
}

async fn execute_admin_user_sync(args: AdminUserSyncArgs) -> Result<()> {
    CliService::section("Admin User Sync");

    let cloud_user = admin_user::CloudUser::from_credentials()?
        .ok_or_else(|| anyhow!("Not logged in. Run 'systemprompt cloud auth login' first."))?;

    CliService::key_value("Cloud User", &cloud_user.email);

    if let Some(profile_name) = &args.profile {
        let database_url = if let Some(url) = &args.database_url {
            url.clone()
        } else {
            let discovery = admin_user::discover_profiles()?;
            discovery
                .profiles
                .into_iter()
                .find(|p| &p.name == profile_name)
                .and_then(|p| p.database_url)
                .ok_or_else(|| {
                    anyhow!(
                        "Profile '{}' not found or has no database_url",
                        profile_name
                    )
                })?
        };

        let result =
            admin_user::sync_admin_to_database(&cloud_user, &database_url, profile_name).await;
        admin_user::print_sync_results(&[result]);
    } else {
        if args.database_url.is_some() {
            return Err(anyhow!("--database-url requires --profile"));
        }

        let results = admin_user::sync_admin_to_all_profiles(&cloud_user, args.verbose).await;
        admin_user::print_sync_results(&results);
    }

    Ok(())
}
