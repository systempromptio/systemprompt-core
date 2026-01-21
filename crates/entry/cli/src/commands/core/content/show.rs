use super::types::ContentDetailOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Content ID or slug")]
    pub identifier: String,

    #[arg(long, help = "Source ID (required when using slug)")]
    pub source: Option<String>,
}

pub async fn execute(
    args: ShowArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContentDetailOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<ContentDetailOutput>> {
    let repo = ContentRepository::new(pool)?;

    let content = if args.identifier.starts_with("content_")
        || args.identifier.contains('-') && args.identifier.len() > 30
    {
        let id = ContentId::new(args.identifier.clone());
        repo.get_by_id(&id)
            .await?
            .ok_or_else(|| anyhow!("Content not found: {}", args.identifier))?
    } else {
        let source_id = args
            .source
            .as_ref()
            .ok_or_else(|| anyhow!("Source ID required when using slug"))?;
        let source = SourceId::new(source_id.clone());
        repo.get_by_source_and_slug(&source, &args.identifier)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Content not found: {} in source {}",
                    args.identifier,
                    source_id
                )
            })?
    };

    let keywords: Vec<String> = content
        .keywords
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let output = ContentDetailOutput {
        id: content.id,
        slug: content.slug,
        title: content.title,
        description: if content.description.is_empty() {
            None
        } else {
            Some(content.description)
        },
        body: content.body,
        author: if content.author.is_empty() {
            None
        } else {
            Some(content.author)
        },
        published_at: Some(content.published_at),
        keywords,
        kind: content.kind,
        image: content.image,
        category_id: content.category_id,
        source_id: content.source_id,
        version_hash: content.version_hash,
        is_public: content.public,
        updated_at: content.updated_at,
    };

    Ok(CommandResult::card(output).with_title("Content Details"))
}
