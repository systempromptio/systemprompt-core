use crate::cli_settings::CliConfig;
use anyhow::Result;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_runtime::AppContext;

use super::types::UserCountOutput;

pub async fn execute(config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    let count = user_service.count().await?;

    let output = UserCountOutput { count };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("User Count");
        CliService::key_value("Total Users", &count.to_string());
    }

    Ok(())
}
