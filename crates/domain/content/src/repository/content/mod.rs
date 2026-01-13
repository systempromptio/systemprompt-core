use crate::error::ContentError;
use crate::models::{Content, CreateContentParams, UpdateContentParams};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

#[derive(Debug)]
pub struct ContentRepository {
    pool: Arc<PgPool>,
}

impl ContentRepository {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        let pool = db
            .pool_arc()
            .map_err(|e| ContentError::InvalidRequest(format!("Database pool error: {e}")))?;
        Ok(Self { pool })
    }

    #[allow(clippy::cognitive_complexity)]
    pub async fn create(&self, params: &CreateContentParams) -> Result<Content, sqlx::Error> {
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
            RETURNING id as "id: ContentId", slug, title, description, body, author,
                      published_at, keywords, kind, image,
                      category_id as "category_id: CategoryId",
                      source_id as "source_id: SourceId",
                      version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                      image_optimization_status, updated_at
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
        .fetch_one(&*self.pool)
        .await
    }

    pub async fn get_by_id(&self, id: &ContentId) -> Result<Option<Content>, sqlx::Error> {
        sqlx::query_as!(
            Content,
            r#"
            SELECT id as "id: ContentId", slug, title, description, body, author,
                   published_at, keywords, kind, image,
                   category_id as "category_id: CategoryId",
                   source_id as "source_id: SourceId",
                   version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                   image_optimization_status, updated_at
            FROM markdown_content
            WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Content>, sqlx::Error> {
        sqlx::query_as!(
            Content,
            r#"
            SELECT id as "id: ContentId", slug, title, description, body, author,
                   published_at, keywords, kind, image,
                   category_id as "category_id: CategoryId",
                   source_id as "source_id: SourceId",
                   version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                   image_optimization_status, updated_at
            FROM markdown_content
            WHERE slug = $1
            "#,
            slug
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn get_by_source_and_slug(
        &self,
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
                   image_optimization_status, updated_at
            FROM markdown_content
            WHERE source_id = $1 AND slug = $2
            "#,
            source_id.as_str(),
            slug
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Content>, sqlx::Error> {
        sqlx::query_as!(
            Content,
            r#"
            SELECT id as "id: ContentId", slug, title, description, body, author,
                   published_at, keywords, kind, image,
                   category_id as "category_id: CategoryId",
                   source_id as "source_id: SourceId",
                   version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                   image_optimization_status, updated_at
            FROM markdown_content
            ORDER BY published_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn list_by_source(&self, source_id: &SourceId) -> Result<Vec<Content>, sqlx::Error> {
        sqlx::query_as!(
            Content,
            r#"
            SELECT id as "id: ContentId", slug, title, description, body, author,
                   published_at, keywords, kind, image,
                   category_id as "category_id: CategoryId",
                   source_id as "source_id: SourceId",
                   version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                   image_optimization_status, updated_at
            FROM markdown_content
            WHERE source_id = $1
            ORDER BY published_at DESC
            "#,
            source_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn update(&self, params: &UpdateContentParams) -> Result<Content, sqlx::Error> {
        let now = Utc::now();
        sqlx::query_as!(
            Content,
            r#"
            UPDATE markdown_content
            SET title = $1, description = $2, body = $3, keywords = $4,
                image = $5, version_hash = $6, updated_at = $7
            WHERE id = $8
            RETURNING id as "id: ContentId", slug, title, description, body, author,
                      published_at, keywords, kind, image,
                      category_id as "category_id: CategoryId",
                      source_id as "source_id: SourceId",
                      version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                      image_optimization_status, updated_at
            "#,
            params.title,
            params.description,
            params.body,
            params.keywords,
            params.image,
            params.version_hash,
            now,
            params.id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
    }

    pub async fn delete(&self, id: &ContentId) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM markdown_content WHERE id = $1", id.as_str())
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_by_source(&self, source_id: &SourceId) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM markdown_content WHERE source_id = $1",
            source_id.as_str()
        )
        .execute(&*self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_all(&self, limit: i64, offset: i64) -> Result<Vec<Content>, sqlx::Error> {
        sqlx::query_as!(
            Content,
            r#"
            SELECT id as "id: ContentId", slug, title, description, body, author,
                   published_at, keywords, kind, image,
                   category_id as "category_id: CategoryId",
                   source_id as "source_id: SourceId",
                   version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                   image_optimization_status, updated_at
            FROM markdown_content
            ORDER BY published_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_popular_content_ids(
        &self,
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
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(ContentId::new).collect())
    }
}
