use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::SessionId;

use crate::models::{EngagementEvent, FeatureExtractionConfig, MlBehavioralFeatures};
use crate::repository::{EngagementRepository, MlFeaturesRepository, SessionRepository};
use crate::AnalyticsSession;

#[derive(Clone, Debug)]
pub struct FeatureExtractionService {
    session_repo: SessionRepository,
    engagement_repo: EngagementRepository,
    ml_repo: MlFeaturesRepository,
    config: FeatureExtractionConfig,
}

impl FeatureExtractionService {
    pub fn new(db_pool: &DbPool, config: FeatureExtractionConfig) -> Result<Self> {
        Ok(Self {
            session_repo: SessionRepository::new(Arc::clone(db_pool)),
            engagement_repo: EngagementRepository::new(db_pool)?,
            ml_repo: MlFeaturesRepository::new(db_pool)?,
            config,
        })
    }

    pub async fn extract_session_features(&self, session_id: &str) -> Result<MlBehavioralFeatures> {
        let session = self
            .session_repo
            .find_by_id(&SessionId::new(session_id.to_string()))
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let engagements = self.engagement_repo.list_by_session(session_id).await?;
        let features = self.compute_features(&session, &engagements);

        self.ml_repo.insert_features(&features).await?;

        Ok(features)
    }

    fn compute_features(
        &self,
        session: &AnalyticsSession,
        engagements: &[EngagementEvent],
    ) -> MlBehavioralFeatures {
        let session_duration = session
            .ended_at
            .zip(session.started_at)
            .map(|(end, start)| (end - start).num_seconds() as i32);

        let avg_scroll_depth = if engagements.is_empty() {
            None
        } else {
            Some(
                engagements
                    .iter()
                    .map(|e| e.max_scroll_depth as f32)
                    .sum::<f32>()
                    / engagements.len() as f32,
            )
        };

        let max_scroll_depth = engagements.iter().map(|e| e.max_scroll_depth).max();
        let total_clicks: i32 = engagements.iter().map(|e| e.click_count).sum();

        let time_pattern_regularity = Self::compute_timing_regularity(engagements);
        let automation_indicators = Self::detect_automation_signals(engagements);
        let headless_indicators = Self::detect_headless_signals(session);

        let mut feature_vector = Vec::with_capacity(32);
        feature_vector.push(session_duration.unwrap_or(0) as f32);
        feature_vector.push(session.request_count.unwrap_or(0) as f32);
        feature_vector.push(engagements.len() as f32);
        feature_vector.push(avg_scroll_depth.unwrap_or(0.0));
        feature_vector.push(max_scroll_depth.unwrap_or(0) as f32);
        feature_vector.push(total_clicks as f32);
        feature_vector.push(time_pattern_regularity.unwrap_or(0.5));
        feature_vector.push(automation_indicators as f32);
        feature_vector.push(headless_indicators as f32);

        if self.config.normalize_features {
            Self::normalize_vector(&mut feature_vector);
        }

        MlBehavioralFeatures {
            id: format!("mlf_{}", uuid::Uuid::new_v4()),
            session_id: session.session_id.clone(),
            fingerprint_hash: session.fingerprint_hash.clone(),
            is_bot: Some(session.is_bot),
            is_human_verified: Some(false),
            label_source: Some("heuristic".to_string()),
            session_duration_seconds: session_duration,
            total_requests: session.request_count,
            unique_pages_visited: Some(engagements.len() as i32),
            avg_time_between_requests_ms: None,
            request_time_variance: None,
            referrer_present: session.referrer_url.as_ref().map(|_| true),
            has_javascript: Some(true),
            accepts_cookies: None,
            viewport_width: None,
            viewport_height: None,
            avg_scroll_depth,
            max_scroll_depth,
            avg_time_on_page_ms: None,
            total_clicks: Some(total_clicks),
            avg_mouse_speed: None,
            mouse_movement_entropy: None,
            time_pattern_regularity,
            request_burst_count: None,
            headless_indicators: Some(headless_indicators),
            automation_indicators: Some(automation_indicators),
            fingerprint_anomaly_score: None,
            feature_vector: Some(feature_vector),
            created_at: chrono::Utc::now(),
        }
    }

    fn compute_timing_regularity(engagements: &[EngagementEvent]) -> Option<f32> {
        if engagements.len() < 2 {
            return None;
        }

        let times: Vec<_> = engagements.iter().map(|e| e.time_on_page_ms).collect();
        let mean = times.iter().sum::<i32>() as f32 / times.len() as f32;
        let variance = times
            .iter()
            .map(|&t| (t as f32 - mean).powi(2))
            .sum::<f32>()
            / times.len() as f32;
        let std_dev = variance.sqrt();
        let cv = if mean > 0.0 { std_dev / mean } else { 1.0 };

        Some((1.0 / (1.0 + cv)).clamp(0.0, 1.0))
    }

    fn detect_automation_signals(engagements: &[EngagementEvent]) -> i32 {
        let mut signals = 0;

        if !engagements.is_empty() {
            let depths: Vec<_> = engagements.iter().map(|e| e.max_scroll_depth).collect();
            let unique: HashSet<_> = depths.iter().collect();
            if unique.len() == 1 && depths.len() > 2 {
                signals += 1;
            }
        }

        if let Some(regularity) = Self::compute_timing_regularity(engagements) {
            if regularity > 0.95 {
                signals += 1;
            }
        }

        if engagements
            .iter()
            .all(|e| e.mouse_move_distance_px.unwrap_or(0) == 0)
            && !engagements.is_empty()
        {
            signals += 1;
        }

        if engagements.iter().any(|e| e.time_on_page_ms < 500) {
            signals += 1;
        }

        signals
    }

    fn detect_headless_signals(session: &AnalyticsSession) -> i32 {
        let mut signals = 0;

        if let Some(ua) = &session.user_agent {
            let ua_lower = ua.to_lowercase();
            if ua_lower.contains("headless") {
                signals += 2;
            }
            if ua_lower.contains("phantomjs") || ua_lower.contains("selenium") {
                signals += 2;
            }
        }

        signals
    }

    fn normalize_vector(vector: &mut Vec<f32>) {
        let magnitude: f32 = vector.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for v in vector.iter_mut() {
                *v /= magnitude;
            }
        }
    }
}
