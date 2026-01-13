use super::types::{ContentSummary, PopularOutput};
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::ContentRepository;
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct PopularArgs {
    #[arg(long, help = "Filter by source ID (required)")]
    pub source: String,

    #[arg(long, default_value = "30", help = "Days to look back")]
    pub days: i64,

    #[arg(long, default_value = "10")]
    pub limit: i64,
}

pub async fn execute(
    args: PopularArgs,
    _config: &CliConfig,
) -> Result<CommandResult<PopularOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ContentRepository::new(ctx.db_pool())?;

    let source = SourceId::new(args.source.clone());
    let days = i32::try_from(args.days).map_err(|_| anyhow!("Days value too large"))?;

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
        source_id: args.source,
        days: args.days,
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
