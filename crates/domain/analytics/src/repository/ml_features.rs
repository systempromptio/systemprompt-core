use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use systemprompt_core_database::DbPool;

use crate::models::{AnomalyThreshold, MlBehavioralFeatures};

#[derive(Clone, Debug)]
pub struct MlFeaturesRepository {
    pool: Arc<PgPool>,
}

impl MlFeaturesRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    #[allow(clippy::cognitive_complexity)]
    pub async fn insert_features(&self, features: &MlBehavioralFeatures) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO ml_behavioral_features (
                id, session_id, fingerprint_hash,
                is_bot, is_human_verified, label_source,
                session_duration_seconds, total_requests, unique_pages_visited,
                avg_time_between_requests_ms, request_time_variance,
                referrer_present, has_javascript, accepts_cookies,
                viewport_width, viewport_height,
                avg_scroll_depth, max_scroll_depth, avg_time_on_page_ms,
                total_clicks, avg_mouse_speed, mouse_movement_entropy,
                time_pattern_regularity, request_burst_count,
                headless_indicators, automation_indicators, fingerprint_anomaly_score,
                feature_vector
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28
            )
            ON CONFLICT (id) DO NOTHING
            "#,
            features.id,
            features.session_id,
            features.fingerprint_hash,
            features.is_bot,
            features.is_human_verified,
            features.label_source,
            features.session_duration_seconds,
            features.total_requests,
            features.unique_pages_visited,
            features.avg_time_between_requests_ms,
            features.request_time_variance,
            features.referrer_present,
            features.has_javascript,
            features.accepts_cookies,
            features.viewport_width,
            features.viewport_height,
            features.avg_scroll_depth,
            features.max_scroll_depth,
            features.avg_time_on_page_ms,
            features.total_clicks,
            features.avg_mouse_speed,
            features.mouse_movement_entropy,
            features.time_pattern_regularity,
            features.request_burst_count,
            features.headless_indicators,
            features.automation_indicators,
            features.fingerprint_anomaly_score,
            features.feature_vector.as_deref()
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_by_session(&self, session_id: &str) -> Result<Option<MlBehavioralFeatures>> {
        let features = sqlx::query_as!(
            MlBehavioralFeatures,
            r#"
            SELECT
                id, session_id, fingerprint_hash,
                is_bot, is_human_verified, label_source,
                session_duration_seconds, total_requests, unique_pages_visited,
                avg_time_between_requests_ms, request_time_variance,
                referrer_present, has_javascript, accepts_cookies,
                viewport_width, viewport_height,
                avg_scroll_depth, max_scroll_depth, avg_time_on_page_ms,
                total_clicks, avg_mouse_speed, mouse_movement_entropy,
                time_pattern_regularity, request_burst_count,
                headless_indicators, automation_indicators, fingerprint_anomaly_score,
                feature_vector, created_at
            FROM ml_behavioral_features
            WHERE session_id = $1
            "#,
            session_id
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(features)
    }

    pub async fn list_labeled_features(
        &self,
        limit: i64,
        is_bot: Option<bool>,
    ) -> Result<Vec<MlBehavioralFeatures>> {
        let features = match is_bot {
            Some(bot_flag) => {
                sqlx::query_as!(
                    MlBehavioralFeatures,
                    r#"
                    SELECT
                        id, session_id, fingerprint_hash,
                        is_bot, is_human_verified, label_source,
                        session_duration_seconds, total_requests, unique_pages_visited,
                        avg_time_between_requests_ms, request_time_variance,
                        referrer_present, has_javascript, accepts_cookies,
                        viewport_width, viewport_height,
                        avg_scroll_depth, max_scroll_depth, avg_time_on_page_ms,
                        total_clicks, avg_mouse_speed, mouse_movement_entropy,
                        time_pattern_regularity, request_burst_count,
                        headless_indicators, automation_indicators, fingerprint_anomaly_score,
                        feature_vector, created_at
                    FROM ml_behavioral_features
                    WHERE is_human_verified = true AND is_bot = $1
                    ORDER BY created_at DESC
                    LIMIT $2
                    "#,
                    bot_flag,
                    limit
                )
                .fetch_all(&*self.pool)
                .await?
            },
            None => {
                sqlx::query_as!(
                    MlBehavioralFeatures,
                    r#"
                    SELECT
                        id, session_id, fingerprint_hash,
                        is_bot, is_human_verified, label_source,
                        session_duration_seconds, total_requests, unique_pages_visited,
                        avg_time_between_requests_ms, request_time_variance,
                        referrer_present, has_javascript, accepts_cookies,
                        viewport_width, viewport_height,
                        avg_scroll_depth, max_scroll_depth, avg_time_on_page_ms,
                        total_clicks, avg_mouse_speed, mouse_movement_entropy,
                        time_pattern_regularity, request_burst_count,
                        headless_indicators, automation_indicators, fingerprint_anomaly_score,
                        feature_vector, created_at
                    FROM ml_behavioral_features
                    WHERE is_human_verified = true
                    ORDER BY created_at DESC
                    LIMIT $1
                    "#,
                    limit
                )
                .fetch_all(&*self.pool)
                .await?
            },
        };

        Ok(features)
    }

    pub async fn update_label(
        &self,
        session_id: &str,
        is_bot: bool,
        label_source: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE ml_behavioral_features
            SET is_bot = $2, is_human_verified = true, label_source = $3
            WHERE session_id = $1
            "#,
            session_id,
            is_bot,
            label_source
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_anomaly_thresholds(&self) -> Result<Vec<AnomalyThreshold>> {
        let thresholds = sqlx::query_as!(
            AnomalyThreshold,
            r#"
            SELECT metric_name, warning_threshold, critical_threshold, description, updated_at
            FROM anomaly_thresholds
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(thresholds)
    }

    pub async fn update_anomaly_threshold(
        &self,
        metric_name: &str,
        warning_threshold: f32,
        critical_threshold: f32,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE anomaly_thresholds
            SET warning_threshold = $2, critical_threshold = $3, updated_at = CURRENT_TIMESTAMP
            WHERE metric_name = $1
            "#,
            metric_name,
            warning_threshold,
            critical_threshold
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
}
