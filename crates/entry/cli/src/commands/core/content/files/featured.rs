use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_runtime::AppContext;

use crate::CliConfig;
use crate::commands::core::files::types::{FeaturedImageOutput, FileSummary};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct FeaturedArgs {
    #[arg(value_name = "CONTENT_ID", help = "Content ID")]
    pub content: String,

    #[arg(long, help = "Set featured image")]
    pub set: Option<String>,
}

pub(super) async fn execute(args: FeaturedArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: FeaturedArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let service = FileRepository::new(pool)?;

    let content_id = ContentId::new(args.content.clone());

    if let Some(file_id_str) = args.set {
        let file_id = FileId::new(file_id_str);
        service.set_featured(&file_id, &content_id).await?;

        let output = FeaturedImageOutput {
            content_id,
            file: None,
            message: "Featured image set successfully".to_owned(),
        };

        return Ok(CommandOutput::card_value("Featured Image Set", &output));
    }

    let file = service.find_featured_image(&content_id).await?;

    let file_summary = file.map(|f| FileSummary {
        id: FileId::new(f.id.to_string()),
        path: f.path,
        public_url: f.public_url,
        mime_type: f.mime_type,
        size_bytes: f.size_bytes,
        ai_content: f.ai_content,
        created_at: f.created_at,
    });

    let message = file_summary.as_ref().map_or_else(
        || "No featured image set".to_owned(),
        |f| format!("Featured image: {}", f.path),
    );

    let output = FeaturedImageOutput {
        content_id,
        file: file_summary,
        message,
    };

    Ok(CommandOutput::card_value("Featured Image", &output))
}
