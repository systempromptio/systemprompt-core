//! `core content popular`: a source's content ranked by distinct human
//! viewers over a parsed duration window.
//!
//! Registered as a leaf under the `core content` group the content entry
//! crate registers; the kernel resolves it to this leaf, with this leaf's
//! descriptor, and lists it after the group's own subcommands. It reads the
//! ranking and the content rows from the analytics report views, so it names
//! no content-capability type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use systemprompt_analytics::ContentAnalyticsRepository;
use systemprompt_cli::kernel::{
    CliCommandLeaf, CliConfig, CommandContext, CommandDescriptor, CommandOutput, DataImpact,
    DbUrlRouting, DescribeCommand, render_result,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;

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

impl DescribeCommand for PopularArgs {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandLeaf for PopularArgs {
    const PARENT: &'static [&'static str] = &["core", "content"];
    const NAME: &'static str = "popular";
    const ABOUT: &'static str = "Get popular content";

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Direct
    }

    fn run(self, ctx: &CommandContext) -> impl Future<Output = Result<()>> + Send {
        async move {
            let result = execute(self, ctx)
                .await
                .context("Failed to get popular content")?;
            render_result(&result, &ctx.cli);
            Ok(())
        }
    }
}

systemprompt_cli::register_cli_command!(leaf PopularArgs);

pub async fn execute(args: PopularArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub async fn execute_with_pool(
    args: PopularArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let source = SourceId::new(args.source);
    let days_i64 = parse_duration(&args.since)?;
    let days = i32::try_from(days_i64).map_err(|_e| anyhow!("Duration too large"))?;

    let items = ContentAnalyticsRepository::new(pool)
        .popular_content(&source, days, args.limit)
        .await?;

    Ok(
        CommandOutput::table_of(vec!["id", "title", "kind", "published_at"], &items)
            .with_title("Popular Content"),
    )
}
