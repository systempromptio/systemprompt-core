use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_content::models::Content;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_provider_contracts::{ContentDataContext, ContentDataProvider};

use crate::prerender::context::PrerenderContext;

const MAX_RETRIES: u32 = 5;
const RETRY_DELAY_MS: u64 = 500;

pub async fn fetch_content_for_source(
    ctx: &PrerenderContext,
    source_name: &str,
    source_id: &str,
) -> Result<Vec<Content>> {
    let repo = ContentRepository::new(&ctx.db_pool)
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("Failed to create content repository")?;
    fetch_with_retries(&repo, source_id, source_name).await
}

async fn fetch_with_retries(
    repo: &ContentRepository,
    source_id_str: &str,
    source_name: &str,
) -> Result<Vec<Content>> {
    let source_id = SourceId::new(source_id_str);
    let mut last_error = None;

    for retry in 0..=MAX_RETRIES {
        match repo.list_by_source(&source_id).await {
            Ok(contents) if !contents.is_empty() => return Ok(contents),
            Ok(_) if retry < MAX_RETRIES => {
                tracing::warn!(source = %source_name, attempt = retry + 1, "No content found, retrying");
                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
            },
            Ok(_) => return Ok(Vec::new()),
            Err(e) => {
                tracing::warn!(source = %source_name, attempt = retry + 1, error = %e, "Query failed");
                last_error = Some(e);
                if retry < MAX_RETRIES {
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                }
            },
        }
    }

    last_error.map_or_else(
        || Ok(Vec::new()),
        |e| Err(anyhow::anyhow!("{}", e)).context("Failed to fetch content after retries"),
    )
}

pub async fn contents_to_json(
    contents: &[Content],
    source_name: &str,
    providers: &[Arc<dyn ContentDataProvider>],
    db_pool: &DbPool,
) -> Vec<serde_json::Value> {
    let mut items: Vec<serde_json::Value> = contents
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "slug": c.slug,
                "title": c.title,
                "description": c.description,
                "content": c.body,
                "author": c.author,
                "published_at": c.published_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                "updated_at": c.updated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                "keywords": c.keywords,
                "content_type": c.kind,
                "image": c.image,
                "category_id": c.category_id,
                "source_id": c.source_id,
                "links": c.links,
            })
        })
        .collect();

    for (item, content) in items.iter_mut().zip(contents.iter()) {
        let content_id = content.id.to_string();

        for provider in providers {
            let applies = provider.applies_to_sources();
            if !applies.is_empty() && !applies.contains(&source_name.to_string()) {
                continue;
            }

            let ctx = ContentDataContext::new(&content_id, source_name, db_pool);

            if let Err(e) = provider.enrich_content(&ctx, item).await {
                tracing::warn!(
                    provider = %provider.provider_id(),
                    content_id = %content_id,
                    error = %e,
                    "Content data provider enrichment failed"
                );
            }
        }
    }

    items
}

pub async fn fetch_popular_ids(
    ctx: &PrerenderContext,
    source_name: &str,
    source_id_str: &str,
) -> Result<Vec<String>> {
    if source_name.is_empty() {
        return Ok(Vec::new());
    }

    let content_repo = ContentRepository::new(&ctx.db_pool)
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("Failed to create content repository for popular IDs")?;

    let source_id = SourceId::new(source_id_str);
    let ids = content_repo
        .get_popular_content_ids(&source_id, 30, 20)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("Failed to get popular content IDs")?;

    Ok(ids.into_iter().map(|id| id.to_string()).collect())
}
