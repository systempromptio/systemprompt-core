use super::types::ContentDetailOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandOutput;
use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_content::{Content, ContentRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, LocaleCode, SourceId};

use crate::context::CommandContext;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Content ID or slug")]
    pub identifier: String,

    #[arg(
        long,
        help = "Source ID — only required when the slug exists in more than one source"
    )]
    pub source: Option<String>,
}

pub async fn execute(args: ShowArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ContentRepository::new(pool)?;
    let locale = LocaleCode::new("en");

    let content = resolve_content(&repo, &args, &locale).await?;

    let keywords: Vec<String> = content
        .keywords
        .split(',')
        .map(|s| s.trim().to_owned())
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

    Ok(CommandOutput::card_value("Content Details", &output))
}

async fn resolve_content(
    repo: &ContentRepository,
    args: &ShowArgs,
    locale: &LocaleCode,
) -> Result<Content> {
    if args.identifier.starts_with("content_")
        || args.identifier.contains('-') && args.identifier.len() > 30
    {
        let id = ContentId::new(args.identifier.clone());
        return repo
            .get_by_id(&id)
            .await?
            .ok_or_else(|| anyhow!("Content not found: {}", args.identifier));
    }

    if let Some(source_id) = args.source.as_ref() {
        let source = SourceId::new(source_id.clone());
        return repo
            .get_by_source_and_slug(&source, &args.identifier, locale)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Content not found: slug '{}' in source '{}'",
                    args.identifier,
                    source_id
                )
            });
    }

    let sources = repo.find_sources_by_slug(&args.identifier, locale).await?;
    match sources.as_slice() {
        [] => Err(anyhow!("No content with slug '{}' found", args.identifier)),
        [only] => repo
            .get_by_source_and_slug(only, &args.identifier, locale)
            .await?
            .ok_or_else(|| anyhow!("Content not found: {}", args.identifier)),
        many => {
            let list = many
                .iter()
                .map(SourceId::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            Err(anyhow!(
                "Slug '{}' exists in multiple sources: [{}]. Re-run with --source <SOURCE> to \
                 disambiguate.",
                args.identifier,
                list
            ))
        },
    }
}
