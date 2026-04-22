use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::services::issue_cowork_exchange_code;
use systemprompt_runtime::AppContext;

use super::types::ExchangeCodeIssuedOutput;
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct IssueCodeArgs {
    #[arg(long, help = "User ID to issue the exchange code for")]
    pub user_id: String,
}

pub async fn execute(
    args: IssueCodeArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ExchangeCodeIssuedOutput>> {
    let ctx = AppContext::new().await?;

    let user_id = UserId::new(args.user_id.trim());
    if user_id.as_str().is_empty() {
        return Err(anyhow!("user_id cannot be empty"));
    }

    let issued = issue_cowork_exchange_code(ctx.db_pool(), &user_id).await?;

    let output = ExchangeCodeIssuedOutput {
        user_id: user_id.clone(),
        code: issued.code.clone(),
        expires_at: issued.expires_at,
        message: format!(
            "Exchange code issued for {user_id}; valid until {}",
            issued.expires_at
        ),
    };

    Ok(CommandResult::text(output).with_title("Cowork Exchange Code"))
}
