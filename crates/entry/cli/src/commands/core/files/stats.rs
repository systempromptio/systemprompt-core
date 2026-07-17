//! `core files stats` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::{FileRepository, FileStats};

use super::types::{CategoryStat, FileCategoryStats, FileStatsOutput};
use crate::CliConfig;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct StatsArgs;

pub(super) async fn execute(args: StatsArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub(super) async fn execute_with_pool(
    _args: StatsArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let service = FileRepository::new(pool)?;

    let stats: FileStats = service.get_stats().await?;

    let output = FileStatsOutput {
        total_files: stats.total_files,
        total_size_bytes: stats.total_size_bytes,
        ai_images_count: stats.ai_images_count,
        by_category: FileCategoryStats {
            images: CategoryStat {
                count: stats.image_count,
                size_bytes: stats.image_size_bytes,
            },
            documents: CategoryStat {
                count: stats.document_count,
                size_bytes: stats.document_size_bytes,
            },
            audio: CategoryStat {
                count: stats.audio_count,
                size_bytes: stats.audio_size_bytes,
            },
            video: CategoryStat {
                count: stats.video_count,
                size_bytes: stats.video_size_bytes,
            },
            other: CategoryStat {
                count: stats.other_count,
                size_bytes: stats.other_size_bytes,
            },
        },
    };

    Ok(CommandOutput::card_value(
        "File Storage Statistics",
        &output,
    ))
}
