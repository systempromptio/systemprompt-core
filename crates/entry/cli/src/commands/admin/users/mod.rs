//! User administration command tree.
//!
//! [`UsersCommands`] groups the user CRUD, search, export, stats, merge, and
//! the `bulk`, `role`, `session`, `ban`, and `webauthn` subcommand trees. On a
//! `--database-url` invocation only the read-only commands are served; write
//! operations require a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apikey;
mod ban;
mod bulk;
mod count;
mod create;
pub(crate) mod delete;
mod export;
mod list;
mod merge;
mod restore;
mod role;
mod search;
mod service_account;
mod session;
mod show;
mod stats;
mod types;
mod update;
mod webauthn;

use crate::context::CommandContext;
use crate::descriptor::DataImpact;
use crate::shared::{CommandOutput, render_result};
use anyhow::{Result, bail};
use clap::Subcommand;

pub use apikey::{ApiKeyCommands, IssueArgs as ApiKeyIssueArgs};
pub use types::{
    BanAddOutput, BanCheckOutput, BanCleanupOutput, BanListOutput, BanRemoveOutput, BanSummary,
    BulkDeleteOutput, BulkUpdateOutput, RoleAssignOutput, SessionCleanupOutput, SessionEndOutput,
    SessionListOutput, SessionSummary, UserActivityOutput, UserCountBreakdownOutput,
    UserCountOutput, UserCreatedOutput, UserDeletedOutput, UserDetailOutput, UserExportItem,
    UserExportOutput, UserListOutput, UserMergeOutput, UserStatsOutput, UserSummary,
    UserUpdatedOutput, WebauthnSetupTokenOutput,
};

#[derive(Debug, Subcommand)]
pub enum UsersCommands {
    #[command(about = "List users with pagination and filtering")]
    List(list::ListArgs),

    #[command(about = "Show detailed user information")]
    Show(show::ShowArgs),

    #[command(about = "Search users by name, email, or full name")]
    Search(search::SearchArgs),

    #[command(about = "Create a new user")]
    Create(create::CreateArgs),

    #[command(about = "Update user fields")]
    Update(update::UpdateArgs),

    #[command(about = "Archive a user (restorable); --purge deletes an archived user")]
    Delete(delete::DeleteArgs),

    #[command(about = "Restore an archived user within the retention window")]
    Restore(restore::RestoreArgs),

    #[command(name = "legal-hold", about = "Place or release a legal hold on a user")]
    LegalHold(restore::LegalHoldArgs),

    #[command(about = "Get total user count")]
    Count(count::CountArgs),

    #[command(about = "Export users to JSON")]
    Export(export::ExportArgs),

    #[command(about = "Show user statistics dashboard")]
    Stats,

    #[command(about = "Merge source user into target user")]
    Merge(merge::MergeArgs),

    #[command(subcommand, about = "Bulk operations on users")]
    Bulk(bulk::BulkCommands),

    #[command(subcommand, about = "Role management commands")]
    Role(role::RoleCommands),

    #[command(subcommand, about = "Session management commands")]
    Session(session::SessionCommands),

    #[command(subcommand, about = "IP ban management commands")]
    Ban(ban::BanCommands),

    #[command(subcommand, about = "WebAuthn credential management commands")]
    Webauthn(webauthn::WebauthnCommands),

    #[command(
        subcommand,
        name = "api-key",
        about = "Personal access token (sp-live-) management"
    )]
    ApiKey(ApiKeyCommands),

    #[command(
        subcommand,
        name = "service-account",
        about = "Service account management"
    )]
    ServiceAccount(service_account::ServiceAccountCommands),
}

pub async fn execute(cmd: UsersCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && matches!(
            cmd,
            UsersCommands::Create(_)
                | UsersCommands::Update(_)
                | UsersCommands::Delete(_)
                | UsersCommands::Restore(_)
                | UsersCommands::LegalHold(_)
                | UsersCommands::Merge(_)
                | UsersCommands::Bulk(_)
                | UsersCommands::Webauthn(_)
                | UsersCommands::ApiKey(_)
                | UsersCommands::ServiceAccount(_)
        )
    {
        bail!("Write operations require full profile context");
    }

    match cmd {
        UsersCommands::Bulk(cmd) => Box::pin(bulk::execute(cmd, ctx)).await,
        UsersCommands::Role(cmd) => Box::pin(role::execute(cmd, ctx)).await,
        UsersCommands::Session(cmd) => Box::pin(session::execute(cmd, ctx)).await,
        UsersCommands::Ban(cmd) => Box::pin(ban::execute(cmd, ctx)).await,
        UsersCommands::Webauthn(cmd) => Box::pin(webauthn::execute(cmd, ctx)).await,
        other => {
            let output = Box::pin(render_output(other, ctx)).await?;
            render_result(&output, &ctx.cli);
            Ok(())
        },
    }
}

async fn render_output(cmd: UsersCommands, ctx: &CommandContext) -> Result<CommandOutput> {
    match cmd {
        UsersCommands::List(args) => Box::pin(list::execute(args, ctx)).await,
        UsersCommands::Show(args) => Box::pin(show::execute(args, ctx)).await,
        UsersCommands::Search(args) => Box::pin(search::execute(args, ctx)).await,
        UsersCommands::Create(args) => Box::pin(create::execute(args, ctx)).await,
        UsersCommands::Update(args) => Box::pin(update::execute(args, ctx)).await,
        UsersCommands::Delete(args) => Box::pin(delete::execute(args, ctx)).await,
        UsersCommands::Restore(args) => Box::pin(restore::execute(args, ctx)).await,
        UsersCommands::LegalHold(args) => Box::pin(restore::execute_legal_hold(args, ctx)).await,
        UsersCommands::Count(args) => Box::pin(count::execute(args, ctx)).await,
        UsersCommands::Export(args) => Box::pin(export::execute(args, ctx)).await,
        UsersCommands::Stats => Box::pin(stats::execute(ctx)).await,
        UsersCommands::Merge(args) => Box::pin(merge::execute(args, ctx)).await,
        UsersCommands::ApiKey(cmd) => Box::pin(apikey::execute(cmd, ctx)).await,
        UsersCommands::ServiceAccount(cmd) => Box::pin(service_account::execute(cmd, ctx)).await,
        UsersCommands::Bulk(_)
        | UsersCommands::Role(_)
        | UsersCommands::Session(_)
        | UsersCommands::Ban(_)
        | UsersCommands::Webauthn(_) => bail!(
            "internal: a users subgroup reached the rendering dispatch, which only serves \
             commands that produce a single output"
        ),
    }
}

impl UsersCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Delete(_)
            | Self::Merge(_)
            | Self::Bulk(bulk::BulkCommands::Delete(_) | bulk::BulkCommands::Update(_))
            | Self::Role(
                role::RoleCommands::Assign(_)
                | role::RoleCommands::Promote(_)
                | role::RoleCommands::Demote(_),
            )
            | Self::Session(session::SessionCommands::Cleanup(_))
            | Self::Ban(ban::BanCommands::Cleanup(_)) => DataImpact::Destructive,
            Self::List(_)
            | Self::Show(_)
            | Self::Search(_)
            | Self::Create(_)
            | Self::Update(_)
            | Self::Restore(_)
            | Self::LegalHold(_)
            | Self::Count(_)
            | Self::Export(_)
            | Self::Stats
            | Self::Session(session::SessionCommands::List(_) | session::SessionCommands::End(_))
            | Self::Ban(
                ban::BanCommands::List(_)
                | ban::BanCommands::Add(_)
                | ban::BanCommands::Remove(_)
                | ban::BanCommands::Check(_),
            )
            | Self::Webauthn(webauthn::WebauthnCommands::GenerateSetupToken(_))
            | Self::ApiKey(
                ApiKeyCommands::Issue(_) | ApiKeyCommands::List(_) | ApiKeyCommands::Revoke(_),
            )
            | Self::ServiceAccount(_) => DataImpact::Preserving,
        }
    }
}
