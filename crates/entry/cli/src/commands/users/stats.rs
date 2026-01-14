use crate::cli_settings::CliConfig;
use anyhow::Result;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_runtime::AppContext;

use super::types::UserStatsOutput;

pub async fn execute(config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    let stats = user_service.get_stats().await?;

    let output = UserStatsOutput {
        total: stats.total,
        created_24h: stats.created_24h,
        created_7d: stats.created_7d,
        created_30d: stats.created_30d,
        active: stats.active,
        suspended: stats.suspended,
        admins: stats.admins,
        anonymous: stats.anonymous,
        bots: stats.bots,
        oldest_user: stats.oldest_user,
        newest_user: stats.newest_user,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("User Statistics");

        CliService::section("Total Users");
        CliService::key_value("Total", &output.total.to_string());
        CliService::key_value("Active", &output.active.to_string());
        CliService::key_value("Suspended", &output.suspended.to_string());

        CliService::section("Growth");
        CliService::key_value("Created (24h)", &output.created_24h.to_string());
        CliService::key_value("Created (7d)", &output.created_7d.to_string());
        CliService::key_value("Created (30d)", &output.created_30d.to_string());

        CliService::section("User Types");
        CliService::key_value("Admins", &output.admins.to_string());
        CliService::key_value("Anonymous", &output.anonymous.to_string());
        CliService::key_value("Bots", &output.bots.to_string());

        if let Some(oldest) = output.oldest_user {
            CliService::key_value("Oldest User", &oldest.format("%Y-%m-%d %H:%M").to_string());
        }
        if let Some(newest) = output.newest_user {
            CliService::key_value("Newest User", &newest.format("%Y-%m-%d %H:%M").to_string());
        }
    }

    Ok(())
}
