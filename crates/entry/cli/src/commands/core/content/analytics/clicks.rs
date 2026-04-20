use crate::cli_settings::CliConfig;
use crate::commands::core::content::types::{ClickRow, ClicksOutput};
use crate::shared::CommandResult;
use anyhow::Result;
use clap::Args;
use systemprompt_content::LinkAnalyticsService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::LinkId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ClicksArgs {
    #[arg(help = "Link ID")]
    pub link_id: String,

    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}

pub async fn execute(args: ClicksArgs, config: &CliConfig) -> Result<CommandResult<ClicksOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ClicksArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<ClicksOutput>> {
    let service = LinkAnalyticsService::new(pool)?;

    let link_id = LinkId::new(args.link_id.clone());
    let clicks = service
        .get_link_clicks(&link_id, Some(args.limit), Some(args.offset))
        .await?;

    let total = clicks.len() as i64;
    let click_rows: Vec<ClickRow> = clicks
        .into_iter()
        .filter_map(|click| {
            Some(ClickRow {
                click_id: click.id.to_string(),
                session_id: click.session_id,
                user_id: click.user_id,
                clicked_at: click.clicked_at?,
                referrer_page: click.referrer_page,
                device_type: click.device_type,
                country: click.country,
                is_conversion: click.is_conversion.unwrap_or(false),
            })
        })
        .collect();

    let output = ClicksOutput {
        link_id,
        clicks: click_rows,
        total,
    };

    Ok(CommandResult::table(output)
        .with_title("Link Clicks")
        .with_columns(vec![
            "click_id".to_string(),
            "session_id".to_string(),
            "clicked_at".to_string(),
            "device_type".to_string(),
            "country".to_string(),
        ]))
}
