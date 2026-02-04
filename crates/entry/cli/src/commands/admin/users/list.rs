use anyhow::Result;
use clap::{Args, ValueEnum};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserRole, UserService, UserStatus};

use super::types::{UserListOutput, UserSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

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
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<CommandResult<UserListOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<UserListOutput>> {
    let user_service = UserService::new(pool)?;

    let users = if let Some(role_filter) = args.role {
        let role: UserRole = role_filter.into();
        user_service.find_by_role(role).await?
    } else {
        user_service.list(args.limit, args.offset).await?
    };

    let users: Vec<_> = if let Some(status_filter) = args.status {
        let status: UserStatus = status_filter.into();
        let status_str = status.as_str();
        users
            .into_iter()
            .filter(|u| u.status.as_deref() == Some(status_str))
            .collect()
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
                status: u.status.clone(),
                roles: u.roles.clone(),
                created_at: u.created_at,
            })
            .collect(),
        total,
        limit: args.limit,
        offset: args.offset,
    };

    Ok(CommandResult::table(output)
        .with_title("Users")
        .with_columns(vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "status".to_string(),
            "roles".to_string(),
        ]))
}
