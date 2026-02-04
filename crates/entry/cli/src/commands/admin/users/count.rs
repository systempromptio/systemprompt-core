use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use super::types::{UserCountBreakdownOutput, UserCountOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct CountArgs {
    #[arg(long, help = "Show breakdown by status and role")]
    pub breakdown: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CountResult {
    Simple(UserCountOutput),
    Breakdown(UserCountBreakdownOutput),
}

pub async fn execute(args: CountArgs, config: &CliConfig) -> Result<CommandResult<CountResult>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: CountArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<CountResult>> {
    let user_service = UserService::new(pool)?;

    if args.breakdown {
        let breakdown = user_service.count_with_breakdown().await?;

        let output = UserCountBreakdownOutput {
            total: breakdown.total,
            by_status: breakdown.by_status,
            by_role: breakdown.by_role,
        };

        Ok(CommandResult::card(CountResult::Breakdown(output)).with_title("User Count Breakdown"))
    } else {
        let count = user_service.count().await?;
        let output = UserCountOutput { count };

        Ok(CommandResult::text(CountResult::Simple(output)).with_title("User Count"))
    }
}
