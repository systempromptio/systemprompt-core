//! Helpers for fetching `Content` rows from the database and enriching them
//! with data contributed by `ContentDataProvider` extensions.

use std::sync::Arc;

use futures::stream::{self, StreamExt};
use systemprompt_content::ContentRepository;
use systemprompt_content::models::Content;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_provider_contracts::{ContentDataContext, ContentDataProvider};

use crate::error::{GeneratorResult, PublishError};
use crate::prerender::context::PrerenderContext;

const MAX_RETRIES: u32 = 5;
const RETRY_DELAY_MS: u64 = 500;

/// Fetch every published `Content` row for the given source, retrying briefly
/// to absorb the eventual-consistency window after a sync.
pub async fn fetch_content_for_source(
    ctx: &PrerenderContext,
    source_name: &str,
    source_id: &SourceId,
) -> GeneratorResult<Vec<Content>> {
    let repo = ContentRepository::new(&ctx.db_pool)
        .map_err(|e| PublishError::other(format!("Failed to create content repository: {e}")))?;
    fetch_with_retries(&repo, source_id, source_name).await
}

async fn fetch_with_retries(
    repo: &ContentRepository,
    source_id: &SourceId,
    source_name: &str,
) -> GeneratorResult<Vec<Content>> {
    let mut last_error = None;

    for retry in 0..=MAX_RETRIES {
        match repo.list_by_source(source_id).await {
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
        |e| {
            Err(PublishError::other(format!(
                "Failed to fetch content after retries: {e}"
            )))
        },
    )
}

/// Convert a slice of `Content` rows to the JSON shape expected by templates,
/// running `ContentDataProvider` enrichment in parallel.
pub async fn contents_to_json(
    contents: &[Content],
    source_name: &str,
    providers: &[Arc<dyn ContentDataProvider>],
    db_pool: &DbPool,
) -> Vec<serde_json::Value> {
    const ENRICHMENT_CONCURRENCY: usize = 8;

    let futures: Vec<_> = contents
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
        .zip(contents.iter())
        .map(|(mut item, content)| {
            let content_id = content.id.to_string();
            async move {
                for provider in providers {
                    let applies = provider.applies_to_sources();
                    if !applies.is_empty() && !applies.contains(&source_name.to_string()) {
                        continue;
                    }

                    let ctx = ContentDataContext::new(&content_id, source_name, db_pool);

                    if let Err(e) = provider.enrich_content(&ctx, &mut item).await {
                        tracing::warn!(
                            provider = %provider.provider_id(),
                            content_id = %content_id,
                            error = %e,
                            "Content data provider enrichment failed"
                        );
                    }
                }

                item
            }
        })
        .collect();

    stream::iter(futures)
        .buffered(ENRICHMENT_CONCURRENCY)
        .collect()
        .await
}

/// Fetch the IDs of the most popular content items for a source over the
/// configured analytics window. Returns the IDs as strings, ready to be
/// embedded directly into template data.
pub async fn fetch_popular_ids(
    ctx: &PrerenderContext,
    source_name: &str,
    source_id: &SourceId,
) -> GeneratorResult<Vec<String>> {
    if source_name.is_empty() {
        return Ok(Vec::new());
    }

    let content_repo = ContentRepository::new(&ctx.db_pool).map_err(|e| {
        PublishError::other(format!(
            "Failed to create content repository for popular IDs: {e}"
        ))
    })?;

    let ids = content_repo
        .get_popular_content_ids(source_id, 30, 20)
        .await
        .map_err(|e| PublishError::other(format!("Failed to get popular content IDs: {e}")))?;

    Ok(ids.into_iter().map(|id| id.to_string()).collect())
}
