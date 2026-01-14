use super::types::{SearchOutput, SearchResultRow};
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::Result;
use clap::Args;
use systemprompt_core_content::{SearchFilters, SearchRequest, SearchService};
use systemprompt_identifiers::CategoryId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(help = "Search query")]
    pub query: String,

    #[arg(long, help = "Filter by source ID")]
    pub source: Option<String>,

    #[arg(long, help = "Filter by category ID")]
    pub category: Option<String>,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}

pub async fn execute(args: SearchArgs, _config: &CliConfig) -> Result<CommandResult<SearchOutput>> {
    let ctx = AppContext::new().await?;
    let service = SearchService::new(ctx.db_pool())?;

    let filters = args.category.as_ref().map(|cat| SearchFilters {
        category_id: Some(CategoryId::new(cat.clone())),
    });

    let request = SearchRequest {
        query: args.query.clone(),
        filters,
        limit: Some(args.limit),
    };

    let response = service.search(&request).await?;

    let results: Vec<SearchResultRow> = response
        .results
        .into_iter()
        .filter(|r| {
            if let Some(ref src) = args.source {
                r.source_id.as_str() == src
            } else {
                true
            }
        })
        .map(|r| SearchResultRow {
            id: r.id,
            slug: r.slug,
            title: r.title,
            description: if r.description.is_empty() {
                None
            } else {
                Some(r.description)
            },
            image: r.image,
            source_id: r.source_id,
            category_id: r.category_id,
        })
        .collect();

    let total = results.len() as i64;

    let output = SearchOutput {
        results,
        total,
        query: args.query,
    };

    Ok(CommandResult::table(output)
        .with_title("Search Results")
        .with_columns(vec![
            "id".to_string(),
            "title".to_string(),
            "slug".to_string(),
            "source_id".to_string(),
        ]))
}
