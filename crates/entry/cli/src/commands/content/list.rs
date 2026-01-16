use super::types::{ContentListOutput, ContentSummary};
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::Result;
use clap::Args;
use systemprompt_core_content::ContentRepository;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by source ID")]
    pub source: Option<String>,

    #[arg(long, help = "Filter by category ID")]
    pub category: Option<String>,

    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}

pub async fn execute(
    args: ListArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContentListOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<ContentListOutput>> {
    let repo = ContentRepository::new(pool)?;

    let items = match &args.source {
        Some(source_id) => {
            let source = SourceId::new(source_id.clone());
            repo.list_by_source(&source).await?
        },
        None => repo.list(args.limit, args.offset).await?,
    };

    let summaries: Vec<ContentSummary> = items
        .into_iter()
        .filter(|c| {
            if let Some(ref cat) = args.category {
                c.category_id
                    .as_ref()
                    .is_some_and(|cid| cid.as_str() == cat)
            } else {
                true
            }
        })
        .map(|c| ContentSummary {
            id: c.id,
            slug: c.slug,
            title: c.title,
            kind: c.kind,
            source_id: c.source_id,
            category_id: c.category_id,
            published_at: Some(c.published_at),
        })
        .collect();

    let total = summaries.len() as i64;

    let output = ContentListOutput {
        items: summaries,
        total,
        limit: args.limit,
        offset: args.offset,
    };

    Ok(CommandResult::table(output)
        .with_title("Content")
        .with_columns(vec![
            "id".to_string(),
            "title".to_string(),
            "kind".to_string(),
            "source_id".to_string(),
            "published_at".to_string(),
        ]))
}
