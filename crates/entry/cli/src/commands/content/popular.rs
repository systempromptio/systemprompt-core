use super::types::{ContentSummary, PopularOutput};
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::ContentRepository;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::AppContext;

fn parse_duration(s: &str) -> Result<i64> {
    let s = s.trim().to_lowercase();
    if s.ends_with('d') {
        s[..s.len() - 1]
            .parse::<i64>()
            .map_err(|_| anyhow!("Invalid duration format: {}", s))
    } else if s.ends_with('w') {
        s[..s.len() - 1]
            .parse::<i64>()
            .map(|w| w * 7)
            .map_err(|_| anyhow!("Invalid duration format: {}", s))
    } else {
        s.parse::<i64>().map_err(|_| {
            anyhow!(
                "Invalid duration format: {}. Use '7d', '30d', '1w', etc.",
                s
            )
        })
    }
}

#[derive(Debug, Args)]
pub struct PopularArgs {
    #[arg(long, help = "Filter by source ID (required)")]
    pub source: String,

    #[arg(long, default_value = "30d", help = "Time period (e.g., 7d, 30d, 1w)")]
    pub since: String,

    #[arg(long, default_value = "10")]
    pub limit: i64,
}

pub async fn execute(
    args: PopularArgs,
    config: &CliConfig,
) -> Result<CommandResult<PopularOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: PopularArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<PopularOutput>> {
    let repo = ContentRepository::new(pool)?;

    let source = SourceId::new(args.source.clone());
    let days_i64 = parse_duration(&args.since)?;
    let days = i32::try_from(days_i64).map_err(|_| anyhow!("Duration too large"))?;

    let content_ids = repo
        .get_popular_content_ids(&source, days, args.limit)
        .await?;

    let mut items = Vec::with_capacity(content_ids.len());
    for id in content_ids {
        if let Some(content) = repo.get_by_id(&id).await? {
            items.push(ContentSummary {
                id: content.id,
                slug: content.slug,
                title: content.title,
                kind: content.kind,
                source_id: content.source_id,
                category_id: content.category_id,
                published_at: Some(content.published_at),
            });
        }
    }

    let output = PopularOutput {
        items,
        source_id: source,
        days: days_i64,
    };

    Ok(CommandResult::table(output)
        .with_title("Popular Content")
        .with_columns(vec![
            "id".to_string(),
            "title".to_string(),
            "kind".to_string(),
            "published_at".to_string(),
        ]))
}
