mod mutations;
mod queries;

use crate::error::ContentError;
use crate::models::{Content, CreateContentParams, UpdateContentParams};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
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

    pub async fn create(&self, params: &CreateContentParams) -> Result<Content, sqlx::Error> {
        mutations::create(&self.pool, params).await
    }

    pub async fn get_by_id(&self, id: &ContentId) -> Result<Option<Content>, sqlx::Error> {
        queries::get_by_id(&self.pool, id).await
    }

    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Content>, sqlx::Error> {
        queries::get_by_slug(&self.pool, slug).await
    }

    pub async fn get_by_source_and_slug(
        &self,
        source_id: &SourceId,
        slug: &str,
    ) -> Result<Option<Content>, sqlx::Error> {
        queries::get_by_source_and_slug(&self.pool, source_id, slug).await
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Content>, sqlx::Error> {
        queries::list(&self.pool, limit, offset).await
    }

    pub async fn list_by_source(&self, source_id: &SourceId) -> Result<Vec<Content>, sqlx::Error> {
        queries::list_by_source(&self.pool, source_id).await
    }

    pub async fn list_by_source_limited(
        &self,
        source_id: &SourceId,
        limit: i64,
    ) -> Result<Vec<Content>, sqlx::Error> {
        queries::list_by_source_limited(&self.pool, source_id, limit).await
    }

    pub async fn update(&self, params: &UpdateContentParams) -> Result<Content, sqlx::Error> {
        mutations::update(&self.pool, params).await
    }

    pub async fn category_exists(&self, category_id: &CategoryId) -> Result<bool, sqlx::Error> {
        queries::category_exists(&self.pool, category_id).await
    }

    pub async fn delete(&self, id: &ContentId) -> Result<(), sqlx::Error> {
        mutations::delete(&self.pool, id).await
    }

    pub async fn delete_by_source(&self, source_id: &SourceId) -> Result<u64, sqlx::Error> {
        mutations::delete_by_source(&self.pool, source_id).await
    }

    pub async fn list_all(&self, limit: i64, offset: i64) -> Result<Vec<Content>, sqlx::Error> {
        queries::list_all(&self.pool, limit, offset).await
    }

    pub async fn get_popular_content_ids(
        &self,
        source_id: &SourceId,
        days: i32,
        limit: i64,
    ) -> Result<Vec<ContentId>, sqlx::Error> {
        queries::get_popular_content_ids(&self.pool, source_id, days, limit).await
    }
}
