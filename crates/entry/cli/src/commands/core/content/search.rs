//! `core content search` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::types::{SearchOutput, SearchResultRow};
use crate::cli_settings::CliConfig;
use crate::shared::CommandOutput;
use anyhow::Result;
use clap::Args;
use systemprompt_content::{SearchFilters, SearchRequest, SearchService};
use systemprompt_database::DbPool;
use systemprompt_identifiers::CategoryId;

use crate::context::CommandContext;

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

pub async fn execute(args: SearchArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub async fn execute_with_pool(
    args: SearchArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let service = SearchService::new(pool)?;

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
            args.source
                .as_ref()
                .is_none_or(|src| r.source_id.as_str() == src)
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

    Ok(
        CommandOutput::table_of(vec!["id", "title", "slug", "source_id"], &output.results)
            .with_title("Search Results"),
    )
}
