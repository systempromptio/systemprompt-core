use crate::models::Content;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

pub async fn get_by_id(pool: &Arc<PgPool>, id: &ContentId) -> Result<Option<Content>, sqlx::Error> {
    sqlx::query_as!(
        Content,
        r#"
        SELECT id as "id: ContentId", slug, title, description, body, author,
               published_at, keywords, kind, image,
               category_id as "category_id: CategoryId",
               source_id as "source_id: SourceId",
               version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
               updated_at
        FROM markdown_content
        WHERE id = $1
        "#,
        id.as_str()
    )
    .fetch_optional(&**pool)
    .await
}

pub async fn get_by_slug(pool: &Arc<PgPool>, slug: &str) -> Result<Option<Content>, sqlx::Error> {
    sqlx::query_as!(
        Content,
        r#"
        SELECT id as "id: ContentId", slug, title, description, body, author,
               published_at, keywords, kind, image,
               category_id as "category_id: CategoryId",
               source_id as "source_id: SourceId",
               version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
               updated_at
        FROM markdown_content
        WHERE slug = $1
        "#,
        slug
    )
    .fetch_optional(&**pool)
    .await
}

pub async fn get_by_source_and_slug(
    pool: &Arc<PgPool>,
    source_id: &SourceId,
    slug: &str,
) -> Result<Option<Content>, sqlx::Error> {
    sqlx::query_as!(
        Content,
        r#"
        SELECT id as "id: ContentId", slug, title, description, body, author,
               published_at, keywords, kind, image,
               category_id as "category_id: CategoryId",
               source_id as "source_id: SourceId",
               version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
               updated_at
        FROM markdown_content
        WHERE source_id = $1 AND slug = $2
        "#,
        source_id.as_str(),
        slug
    )
    .fetch_optional(&**pool)
    .await
}

pub async fn list(
    pool: &Arc<PgPool>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Content>, sqlx::Error> {
    sqlx::query_as!(
        Content,
        r#"
        SELECT id as "id: ContentId", slug, title, description, body, author,
               published_at, keywords, kind, image,
               category_id as "category_id: CategoryId",
               source_id as "source_id: SourceId",
               version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
               updated_at
        FROM markdown_content
        ORDER BY published_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset
    )
    .fetch_all(&**pool)
    .await
}

pub async fn list_by_source(
    pool: &Arc<PgPool>,
    source_id: &SourceId,
) -> Result<Vec<Content>, sqlx::Error> {
    sqlx::query_as!(
        Content,
        r#"
        SELECT id as "id: ContentId", slug, title, description, body, author,
               published_at, keywords, kind, image,
               category_id as "category_id: CategoryId",
               source_id as "source_id: SourceId",
               version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
               updated_at
        FROM markdown_content
        WHERE source_id = $1
        ORDER BY published_at DESC
        "#,
        source_id.as_str()
    )
    .fetch_all(&**pool)
    .await
}

pub async fn list_by_source_limited(
    pool: &Arc<PgPool>,
    source_id: &SourceId,
    limit: i64,
) -> Result<Vec<Content>, sqlx::Error> {
    sqlx::query_as!(
        Content,
        r#"
        SELECT id as "id: ContentId", slug, title, description, body, author,
               published_at, keywords, kind, image,
               category_id as "category_id: CategoryId",
               source_id as "source_id: SourceId",
               version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
               updated_at
        FROM markdown_content
        WHERE source_id = $1
        ORDER BY published_at DESC
        LIMIT $2
        "#,
        source_id.as_str(),
        limit
    )
    .fetch_all(&**pool)
    .await
}

pub async fn list_all(
    pool: &Arc<PgPool>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Content>, sqlx::Error> {
    sqlx::query_as!(
        Content,
        r#"
        SELECT id as "id: ContentId", slug, title, description, body, author,
               published_at, keywords, kind, image,
               category_id as "category_id: CategoryId",
               source_id as "source_id: SourceId",
               version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
               updated_at
        FROM markdown_content
        ORDER BY published_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset
    )
    .fetch_all(&**pool)
    .await
}

pub async fn category_exists(
    pool: &Arc<PgPool>,
    category_id: &CategoryId,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM markdown_categories WHERE id = $1) as "exists!""#,
        category_id.as_str()
    )
    .fetch_one(&**pool)
    .await?;
    Ok(result)
}

pub async fn get_popular_content_ids(
    pool: &Arc<PgPool>,
    source_id: &SourceId,
    days: i32,
    limit: i64,
) -> Result<Vec<ContentId>, sqlx::Error> {
    let rows = sqlx::query_scalar!(
        r#"
        SELECT mc.id as "id!"
        FROM markdown_content mc
        LEFT JOIN analytics_events ae ON
            ae.event_type = 'page_view'
            AND ae.event_category = 'content'
            AND ae.endpoint = 'GET /' || mc.source_id || '/' || mc.slug
            AND ae.timestamp >= CURRENT_TIMESTAMP - ($2 || ' days')::INTERVAL
        LEFT JOIN users u ON ae.user_id = u.id
        WHERE mc.source_id = $1
        GROUP BY mc.id, mc.published_at
        ORDER BY COUNT(DISTINCT CASE
            WHEN u.id IS NOT NULL AND u.is_bot = FALSE AND u.is_scanner = FALSE
            THEN ae.user_id
        END) DESC, mc.published_at DESC
        LIMIT $3
        "#,
        source_id.as_str(),
        days.to_string(),
        limit
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows.into_iter().map(ContentId::new).collect())
}
