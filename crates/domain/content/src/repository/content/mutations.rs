use crate::models::{Content, ContentKind, CreateContentParams, UpdateContentParams};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

use super::queries;

#[allow(clippy::cognitive_complexity)]
pub async fn create(
    pool: &Arc<PgPool>,
    params: &CreateContentParams,
) -> Result<Content, sqlx::Error> {
    let id = ContentId::new(uuid::Uuid::new_v4().to_string());
    let now = Utc::now();
    sqlx::query_as!(
        Content,
        r#"
        INSERT INTO markdown_content (
            id, slug, title, description, body, author,
            published_at, keywords, kind, image, category_id, source_id,
            version_hash, links, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (slug) DO UPDATE SET
            title = EXCLUDED.title,
            description = EXCLUDED.description,
            body = EXCLUDED.body,
            author = EXCLUDED.author,
            published_at = EXCLUDED.published_at,
            keywords = EXCLUDED.keywords,
            kind = EXCLUDED.kind,
            image = EXCLUDED.image,
            category_id = EXCLUDED.category_id,
            source_id = EXCLUDED.source_id,
            version_hash = EXCLUDED.version_hash,
            links = EXCLUDED.links,
            updated_at = EXCLUDED.updated_at
        RETURNING id as "id: ContentId", slug, title, description, body, author,
                  published_at, keywords, kind, image,
                  category_id as "category_id: CategoryId",
                  source_id as "source_id: SourceId",
                  version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                  updated_at
        "#,
        id.as_str(),
        params.slug,
        params.title,
        params.description,
        params.body,
        params.author,
        params.published_at,
        params.keywords,
        params.kind.as_str(),
        params.image,
        params.category_id.as_ref().map(CategoryId::as_str),
        params.source_id.as_str(),
        params.version_hash,
        params.links,
        now
    )
    .fetch_one(&**pool)
    .await
}

pub async fn update(
    pool: &Arc<PgPool>,
    params: &UpdateContentParams,
) -> Result<Content, sqlx::Error> {
    let now = Utc::now();

    let category_id_value: Option<String> = match &params.category_id {
        Some(Some(cat)) => Some(cat.as_str().to_string()),
        Some(None) => None,
        None => {
            let current = queries::get_by_id(pool, &params.id).await?;
            current.and_then(|c| c.category_id.map(|cat| cat.as_str().to_string()))
        },
    };

    let kind_value: String = if let Some(k) = &params.kind {
        k.clone()
    } else {
        let current = queries::get_by_id(pool, &params.id).await?;
        current.map_or_else(|| ContentKind::Article.as_str().to_string(), |c| c.kind)
    };

    let public_value: bool = if let Some(p) = params.public {
        p
    } else {
        let current = queries::get_by_id(pool, &params.id).await?;
        current.is_some_and(|c| c.public)
    };

    sqlx::query_as!(
        Content,
        r#"
        UPDATE markdown_content
        SET title = $1, description = $2, body = $3, keywords = $4,
            image = $5, version_hash = $6, updated_at = $7,
            category_id = $8, kind = $9, public = $10
        WHERE id = $11
        RETURNING id as "id: ContentId", slug, title, description, body, author,
                  published_at, keywords, kind, image,
                  category_id as "category_id: CategoryId",
                  source_id as "source_id: SourceId",
                  version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                  updated_at
        "#,
        params.title,
        params.description,
        params.body,
        params.keywords,
        params.image,
        params.version_hash,
        now,
        category_id_value,
        kind_value,
        public_value,
        params.id.as_str()
    )
    .fetch_one(&**pool)
    .await
}

pub async fn delete(pool: &Arc<PgPool>, id: &ContentId) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM markdown_content WHERE id = $1", id.as_str())
        .execute(&**pool)
        .await?;
    Ok(())
}

pub async fn delete_by_source(
    pool: &Arc<PgPool>,
    source_id: &SourceId,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        "DELETE FROM markdown_content WHERE source_id = $1",
        source_id.as_str()
    )
    .execute(&**pool)
    .await?;
    Ok(result.rows_affected())
}
