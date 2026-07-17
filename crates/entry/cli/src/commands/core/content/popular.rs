//! `core content popular` command over a parsed duration window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::types::{ContentSummary, PopularOutput};
use crate::cli_settings::CliConfig;
use crate::shared::CommandOutput;
use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;

use crate::context::CommandContext;

fn parse_duration(s: &str) -> Result<i64> {
    let s = s.trim().to_lowercase();
    if s.ends_with('d') {
        s[..s.len() - 1]
            .parse::<i64>()
            .map_err(|_e| anyhow!("Invalid duration format: {}", s))
    } else if s.ends_with('w') {
        s[..s.len() - 1]
            .parse::<i64>()
            .map(|w| w * 7)
            .map_err(|_e| anyhow!("Invalid duration format: {}", s))
    } else {
        s.parse::<i64>().map_err(|_e| {
            anyhow!(
                "Invalid duration format: {}. Use '7d', '30d', '1w', etc.",
                s
            )
        })
    }
}

#[derive(Debug, Args)]
pub struct PopularArgs {
    #[arg(help = "Source ID")]
    pub source: String,

    #[arg(long, default_value = "30d", help = "Time period (e.g., 7d, 30d, 1w)")]
    pub since: String,

    #[arg(long, default_value = "10")]
    pub limit: i64,
}

pub async fn execute(args: PopularArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub async fn execute_with_pool(
    args: PopularArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ContentRepository::new(pool)?;

    let source = SourceId::new(args.source.clone());
    let days_i64 = parse_duration(&args.since)?;
    let days = i32::try_from(days_i64).map_err(|_e| anyhow!("Duration too large"))?;

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

    Ok(
        CommandOutput::table_of(vec!["id", "title", "kind", "published_at"], &output.items)
            .with_title("Popular Content"),
    )
}
