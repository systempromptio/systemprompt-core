//! `cloud auth` subcommands: login, logout, and whoami.
//!
//! Dispatches the [`AuthCommands`] enum to the per-command modules that manage
//! the locally stored cloud credentials and authenticated-user state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod login;
mod logout;
mod operator;
mod whoami;

pub use login::{build_login_output, complete_login};

use crate::context::CommandContext;
use crate::shared::{parse_client_id, render_result};
use anyhow::Result;
use clap::{Args, Subcommand};

use super::Environment;
use systemprompt_identifiers::ClientId;

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCommands {
    #[command(
        about = "Authenticate with systemprompt.io Cloud via OAuth, or sign in as an operator \
                 against an OIDC issuer (--issuer, --client-id) for self-managed tenants"
    )]
    Login {
        #[arg(value_enum, default_value_t = Environment::default(), hide = true)]
        environment: Environment,

        #[arg(
            long,
            help = "OIDC issuer URL for operator sign-in to self-managed tenants"
        )]
        issuer: Option<String>,

        #[arg(
            long,
            value_parser = parse_client_id,
            help = "OAuth client id registered at the operator issuer"
        )]
        client_id: Option<ClientId>,
    },

    #[command(about = "Clear saved cloud credentials")]
    Logout(LogoutArgs),

    #[command(
        about = "Show current authenticated user and token status",
        alias = "status"
    )]
    Whoami,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct LogoutArgs {
    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(cmd: AuthCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        AuthCommands::Login {
            environment,
            issuer,
            client_id,
        } => {
            let result = match operator::operator_request(issuer, client_id, ctx.profile().ok())? {
                Some(request) => operator::execute(&request, &ctx.cli).await?,
                None => login::execute(environment, ctx.prompter(), &ctx.cli).await?,
            };
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AuthCommands::Logout(args) => {
            let result = logout::execute(args, ctx.prompter(), &ctx.cli)?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AuthCommands::Whoami => {
            let result = whoami::execute(&ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
