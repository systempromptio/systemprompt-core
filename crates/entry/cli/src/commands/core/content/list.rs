use super::types::{ContentListOutput, ContentSummary};
use crate::cli_settings::CliConfig;
use crate::shared::CommandOutput;
use anyhow::Result;
use clap::Args;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{LocaleCode, SourceId};
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

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ContentRepository::new(pool)?;

    let items = match &args.source {
        Some(source_id) => {
            let source = SourceId::new(source_id.clone());
            repo.list_by_source(&source, &LocaleCode::new("en")).await?
        },
        None => repo.list(args.limit, args.offset).await?,
    };

    let summaries: Vec<ContentSummary> = items
        .into_iter()
        .filter(|c| {
            args.category.as_ref().is_none_or(|cat| {
                c.category_id
                    .as_ref()
                    .is_some_and(|cid| cid.as_str() == cat)
            })
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

    Ok(CommandOutput::table_of(
        vec!["id", "title", "kind", "source_id", "published_at"],
        &output.items,
    )
    .with_title("Content"))
}
