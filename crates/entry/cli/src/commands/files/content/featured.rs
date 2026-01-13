use anyhow::Result;
use clap::Args;
use systemprompt_core_files::ContentService;
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_runtime::AppContext;

use crate::commands::files::types::{FeaturedImageOutput, FileSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct FeaturedArgs {
    #[arg(help = "Content ID")]
    pub content_id: String,

    #[arg(long, help = "Set featured image")]
    pub set: Option<String>,
}

pub async fn execute(
    args: FeaturedArgs,
    _config: &CliConfig,
) -> Result<CommandResult<FeaturedImageOutput>> {
    let ctx = AppContext::new().await?;
    let service = ContentService::new(ctx.db_pool())?;

    let content_id = ContentId::new(args.content_id.clone());

    if let Some(file_id_str) = args.set {
        let file_id = FileId::new(file_id_str);
        service.set_featured(&file_id, &content_id).await?;

        let output = FeaturedImageOutput {
            content_id,
            file: None,
            message: "Featured image set successfully".to_string(),
        };

        return Ok(CommandResult::card(output).with_title("Featured Image Set"));
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

    let message = file_summary
        .as_ref()
        .map_or("No featured image set".to_string(), |f| {
            format!("Featured image: {}", f.path)
        });

    let output = FeaturedImageOutput {
        content_id,
        file: file_summary,
        message,
    };

    Ok(CommandResult::card(output).with_title("Featured Image"))
}
