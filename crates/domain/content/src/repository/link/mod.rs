pub mod analytics;

pub use analytics::LinkAnalyticsRepository;

use crate::error::ContentError;
use crate::models::{CampaignLink, CreateLinkParams};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CampaignId, ContentId, LinkId};

#[derive(Debug)]
pub struct LinkRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl LinkRepository {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        let pool = db
            .pool_arc()
            .map_err(|e| ContentError::InvalidRequest(format!("Database pool error: {e}")))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| ContentError::InvalidRequest(format!("Database write pool error: {e}")))?;
        Ok(Self { pool, write_pool })
    }

    #[allow(clippy::cognitive_complexity)]
    pub async fn create_link(
        &self,
        params: &CreateLinkParams,
    ) -> Result<CampaignLink, sqlx::Error> {
        let id = LinkId::generate();
        let now = Utc::now();
        sqlx::query_as!(
            CampaignLink,
            r#"
            INSERT INTO campaign_links (
                id, short_code, target_url, link_type, source_content_id, source_page,
                campaign_id, campaign_name, utm_params, link_text, link_position,
                destination_type, is_active, expires_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $15)
            ON CONFLICT (short_code) DO UPDATE SET
                target_url = EXCLUDED.target_url,
                link_type = EXCLUDED.link_type,
                source_content_id = EXCLUDED.source_content_id,
                source_page = EXCLUDED.source_page,
                campaign_id = EXCLUDED.campaign_id,
                campaign_name = EXCLUDED.campaign_name,
                utm_params = EXCLUDED.utm_params,
                link_text = EXCLUDED.link_text,
                link_position = EXCLUDED.link_position,
                destination_type = EXCLUDED.destination_type,
                is_active = EXCLUDED.is_active,
                expires_at = EXCLUDED.expires_at,
                updated_at = EXCLUDED.updated_at
            RETURNING id as "id: LinkId", short_code, target_url, link_type,
                      campaign_id as "campaign_id: CampaignId", campaign_name,
                      source_content_id as "source_content_id: ContentId", source_page,
                      utm_params, link_text, link_position, destination_type,
                      click_count, unique_click_count, conversion_count,
                      is_active, expires_at, created_at, updated_at
            "#,
            id.as_str(),
            params.short_code,
            params.target_url,
            params.link_type,
            params.source_content_id.as_ref().map(ContentId::as_str),
            params.source_page,
            params.campaign_id.as_ref().map(CampaignId::as_str),
            params.campaign_name,
            params.utm_params,
            params.link_text,
            params.link_position,
            params.destination_type,
            params.is_active,
            params.expires_at,
            now
        )
        .fetch_one(&*self.write_pool)
        .await
    }

    pub async fn get_link_by_short_code(
        &self,
        short_code: &str,
    ) -> Result<Option<CampaignLink>, sqlx::Error> {
        sqlx::query_as!(
            CampaignLink,
            r#"
            SELECT id as "id: LinkId", short_code, target_url, link_type,
                   campaign_id as "campaign_id: CampaignId", campaign_name,
                   source_content_id as "source_content_id: ContentId", source_page,
                   utm_params, link_text, link_position, destination_type,
                   click_count, unique_click_count, conversion_count,
                   is_active, expires_at, created_at, updated_at
            FROM campaign_links
            WHERE short_code = $1 AND is_active = true
            "#,
            short_code
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn list_links_by_campaign(
        &self,
        campaign_id: &CampaignId,
    ) -> Result<Vec<CampaignLink>, sqlx::Error> {
        sqlx::query_as!(
            CampaignLink,
            r#"
            SELECT id as "id: LinkId", short_code, target_url, link_type,
                   campaign_id as "campaign_id: CampaignId", campaign_name,
                   source_content_id as "source_content_id: ContentId", source_page,
                   utm_params, link_text, link_position, destination_type,
                   click_count, unique_click_count, conversion_count,
                   is_active, expires_at, created_at, updated_at
            FROM campaign_links
            WHERE campaign_id = $1
            ORDER BY created_at DESC
            "#,
            campaign_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn list_links_by_source_content(
        &self,
        content_id: &ContentId,
    ) -> Result<Vec<CampaignLink>, sqlx::Error> {
        sqlx::query_as!(
            CampaignLink,
            r#"
            SELECT id as "id: LinkId", short_code, target_url, link_type,
                   campaign_id as "campaign_id: CampaignId", campaign_name,
                   source_content_id as "source_content_id: ContentId", source_page,
                   utm_params, link_text, link_position, destination_type,
                   click_count, unique_click_count, conversion_count,
                   is_active, expires_at, created_at, updated_at
            FROM campaign_links
            WHERE source_content_id = $1
            ORDER BY created_at DESC
            "#,
            content_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_link_by_id(&self, id: &LinkId) -> Result<Option<CampaignLink>, sqlx::Error> {
        sqlx::query_as!(
            CampaignLink,
            r#"
            SELECT id as "id: LinkId", short_code, target_url, link_type,
                   campaign_id as "campaign_id: CampaignId", campaign_name,
                   source_content_id as "source_content_id: ContentId", source_page,
                   utm_params, link_text, link_position, destination_type,
                   click_count, unique_click_count, conversion_count,
                   is_active, expires_at, created_at, updated_at
            FROM campaign_links
            WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn find_link_by_source_and_target(
        &self,
        source_page: &str,
        target_url: &str,
    ) -> Result<Option<CampaignLink>, sqlx::Error> {
        sqlx::query_as!(
            CampaignLink,
            r#"
            SELECT id as "id: LinkId", short_code, target_url, link_type,
                   campaign_id as "campaign_id: CampaignId", campaign_name,
                   source_content_id as "source_content_id: ContentId", source_page,
                   utm_params, link_text, link_position, destination_type,
                   click_count, unique_click_count, conversion_count,
                   is_active, expires_at, created_at, updated_at
            FROM campaign_links
            WHERE source_page = $1 AND target_url = $2 AND is_active = true
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            source_page,
            target_url
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn delete_link(&self, id: &LinkId) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM campaign_links WHERE id = $1", id.as_str())
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
