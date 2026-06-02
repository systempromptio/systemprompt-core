use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use super::types::{UserListOutput, UserSummary};
use crate::CliConfig;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: String,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}

pub(super) async fn execute(args: SearchArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub(super) async fn execute_with_pool(
    args: SearchArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let user_service = UserService::new(pool)?;

    let users = user_service.search(&args.query, args.limit).await?;
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
        offset: 0,
    };

    Ok(CommandOutput::table_of(
        vec!["id", "name", "email", "status", "roles"],
        &output.users,
    )
    .with_title("User Search Results"))
}
