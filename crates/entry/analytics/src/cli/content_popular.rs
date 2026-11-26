//! `core content popular`: a source's content ranked by distinct human
//! viewers over a parsed duration window.
//!
//! Contributed as a leaf under the kernel's `core content` group; the kernel
//! resolves it to this leaf, with this leaf's descriptor, and lists it after
//! the group's own subcommands. It reads the source's content rows from the
//! content member's report port and ranks them by the distinct human viewers
//! the analytics views count.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use systemprompt_analytics::ContentAudienceRepository;
use systemprompt_database::{DbPool, pool_from_handle};
use systemprompt_extension::cli::parsers::parse_source_id;
use systemprompt_extension::cli::{
    CliCommandLeaf, CliConfig, CommandDescriptor, CommandError, CommandHost, CommandOutput,
    DataImpact, DescribeCommand,
};
use systemprompt_identifiers::SourceId;
use systemprompt_traits::usage_reports::content::{
    ContentAudienceCounts, ContentAudienceReport, DynContentAudienceReport, rank_by_viewers,
    source_endpoint_prefix,
};

use crate::cli::shared::report_port;

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
    #[arg(help = "Source ID", value_parser = parse_source_id)]
    pub source: SourceId,

    #[arg(long, default_value = "30d", help = "Time period (e.g., 7d, 30d, 1w)")]
    pub since: String,

    #[arg(long, default_value = "10")]
    pub limit: i64,
}

impl DescribeCommand for PopularArgs {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandLeaf for PopularArgs {
    const NAME: &'static str = "popular";
    const ABOUT: &'static str = "Get popular content";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host)
            .await
            .context("Failed to get popular content")
            .map_err(CommandError::new)
    }
}

pub async fn execute(args: PopularArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let content =
        report_port::<DynContentAudienceReport>(host, "core content popular", "content").await?;
    let pool = pool_from_handle(host.db().await?)?;
    execute_with_pool(args, &pool, content.as_ref(), host.output()).await
}

pub async fn execute_with_pool(
    args: PopularArgs,
    pool: &DbPool,
    content: &dyn ContentAudienceReport,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let days_i64 = parse_duration(&args.since)?;
    let days = i32::try_from(days_i64).map_err(|_e| anyhow!("Duration too large"))?;

    let limit = usize::try_from(args.limit).map_err(|_e| anyhow!("Invalid limit"))?;

    let viewers = ContentAudienceRepository::new(pool)
        .endpoint_viewers(&source_endpoint_prefix(&args.source), days)
        .await?;
    let items = rank_by_viewers(content.source_content(&args.source).await?, &viewers, limit);

    Ok(
        CommandOutput::table_of(vec!["id", "title", "kind", "published_at"], &items)
            .with_title("Popular Content"),
    )
}
