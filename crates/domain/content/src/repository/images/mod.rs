use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::ContentId;

#[derive(Debug, Clone)]
pub struct ImageRepository {
    pool: Arc<PgPool>,
}

#[derive(Debug)]
pub struct UnoptimizedImage {
    pub id: ContentId,
    pub image: Option<String>,
}

impl ImageRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn find_unoptimized_images(&self, limit: i64) -> Result<Vec<UnoptimizedImage>> {
        let rows = sqlx::query_as!(
            UnoptimizedImage,
            r#"SELECT id as "id: ContentId", image FROM markdown_content WHERE
             image IS NOT NULL AND image != '' AND
             (image_optimization_status IS NULL OR image_optimization_status != 'optimized')
             ORDER BY published_at DESC LIMIT $1"#,
            limit
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context("Failed to fetch unoptimized images")?;

        Ok(rows)
    }

    pub async fn update_image_url(
        &self,
        content_id: &ContentId,
        new_image_url: &str,
    ) -> Result<()> {
        let now = Utc::now();
        sqlx::query!(
            r#"UPDATE markdown_content SET image = $1, image_optimization_status = 'optimized',
             updated_at = $2 WHERE id = $3"#,
            new_image_url,
            now,
            content_id.as_str()
        )
        .execute(self.pool.as_ref())
        .await
        .context("Failed to update content image")?;

        Ok(())
    }
}
