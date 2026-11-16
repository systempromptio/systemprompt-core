//! `admin users list` command with role/status filters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, ValueEnum};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_users::{UserRepository, UserRole, UserService, UserStatus};

use crate::CliConfig;
use crate::shared::CommandOutput;
use super::types::{UserListOutput, UserSummary};
use systemprompt_extension::cli::CommandHost;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RoleFilter {
    Admin,
    User,
    Anonymous,
}

impl From<RoleFilter> for UserRole {
    fn from(filter: RoleFilter) -> Self {
        match filter {
            RoleFilter::Admin => Self::Admin,
            RoleFilter::User => Self::User,
            RoleFilter::Anonymous => Self::Anonymous,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    Active,
    Inactive,
    Suspended,
    Pending,
    Deleted,
    Temporary,
}

impl From<StatusFilter> for UserStatus {
    fn from(filter: StatusFilter) -> Self {
        match filter {
            StatusFilter::Active => Self::Active,
            StatusFilter::Inactive => Self::Inactive,
            StatusFilter::Suspended => Self::Suspended,
            StatusFilter::Pending => Self::Pending,
            StatusFilter::Deleted => Self::Deleted,
            StatusFilter::Temporary => Self::Temporary,
        }
    }
}

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, value_enum)]
    pub role: Option<RoleFilter>,

    #[arg(long, value_enum)]
    pub status: Option<StatusFilter>,

    #[arg(long)]
    pub include_anonymous: bool,
}

pub(super) async fn execute(args: ListArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    execute_with_pool(args, &systemprompt_database::pool_from_handle(host.db().await?)?, host.output()).await
}

pub(super) async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let user_service = UserService::new(Arc::new(UserRepository::new(pool)));

    let users = if let Some(role_filter) = args.role {
        let role: UserRole = role_filter.into();
        user_service.list_by_role(role).await?
    } else if args.include_anonymous {
        user_service
            .list_including_anonymous(args.limit, args.offset)
            .await?
    } else {
        user_service.list(args.limit, args.offset).await?
    };

    let users: Vec<_> = if let Some(status_filter) = args.status {
        let status: UserStatus = status_filter.into();
        users.into_iter().filter(|u| u.status == status).collect()
    } else {
        users
    };

    let total = users.len() as i64;

    let output = UserListOutput {
        users: users
            .iter()
            .map(|u| UserSummary {
                id: u.id.clone(),
                name: u.name.clone(),
                email: u.email.clone(),
                status: u.status.to_string(),
                roles: u.roles.clone(),
                created_at: u.created_at,
            })
            .collect(),
        total,
        limit: args.limit,
        offset: args.offset,
    };

    Ok(CommandOutput::table_of(
        vec!["id", "name", "email", "status", "roles"],
        &output.users,
    )
    .with_title("Users"))
}
