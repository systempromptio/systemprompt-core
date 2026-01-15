use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_runtime::AppContext;

use super::types::{UserCountBreakdownOutput, UserCountOutput};

#[derive(Debug, Clone, Copy, Args)]
pub struct CountArgs {
    #[arg(long, help = "Show breakdown by status and role")]
    pub breakdown: bool,
}

pub async fn execute(args: CountArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if args.breakdown {
        let breakdown = user_service.count_with_breakdown().await?;

        let output = UserCountBreakdownOutput {
            total: breakdown.total,
            by_status: breakdown.by_status,
            by_role: breakdown.by_role,
        };

        if config.is_json_output() {
            CliService::json(&output);
        } else {
            CliService::section("User Count");
            CliService::key_value("Total Users", &output.total.to_string());

            CliService::section("By Status");
            for (status, count) in &output.by_status {
                CliService::key_value(status, &count.to_string());
            }

            CliService::section("By Role");
            for (role, count) in &output.by_role {
                CliService::key_value(role, &count.to_string());
            }
        }
    } else {
        let count = user_service.count().await?;
        let output = UserCountOutput { count };

        if config.is_json_output() {
            CliService::json(&output);
        } else {
            CliService::section("User Count");
            CliService::key_value("Total Users", &count.to_string());
        }
    }

    Ok(())
}
