use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use super::types::{UserCountBreakdownOutput, UserCountOutput};
use crate::CliConfig;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct CountArgs {
    #[arg(long, help = "Show breakdown by status and role")]
    pub breakdown: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum CountResult {
    Simple(UserCountOutput),
    Breakdown(UserCountBreakdownOutput),
}

pub(super) async fn execute(args: CountArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub(super) async fn execute_with_pool(
    args: CountArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let user_service = UserService::new(pool)?;

    if args.breakdown {
        let breakdown = user_service.count_with_breakdown().await?;

        let output = UserCountBreakdownOutput {
            total: breakdown.total,
            by_status: breakdown.by_status,
            by_role: breakdown.by_role,
        };

        Ok(CommandOutput::card_value(
            "User Count Breakdown",
            &CountResult::Breakdown(output),
        ))
    } else {
        let count = user_service.count().await?;
        let output = UserCountOutput { count };

        Ok(CommandOutput::card_value(
            "User Count",
            &CountResult::Simple(output),
        ))
    }
}
