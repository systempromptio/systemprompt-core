use anyhow::Result;
use clap::Args;
use systemprompt_core_files::{FileService, FileStats};
use systemprompt_runtime::AppContext;

use super::types::{CategoryStat, FileCategoryStats, FileStatsOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct StatsArgs;

pub async fn execute(
    _args: StatsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<FileStatsOutput>> {
    let ctx = AppContext::new().await?;
    let service = FileService::new(ctx.db_pool())?;

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

    Ok(CommandResult::card(output).with_title("File Storage Statistics"))
}
