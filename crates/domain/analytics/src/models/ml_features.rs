use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MlBehavioralFeatures {
    pub id: String,
    pub session_id: String,
    pub fingerprint_hash: Option<String>,
    pub is_bot: Option<bool>,
    pub is_human_verified: Option<bool>,
    pub label_source: Option<String>,
    pub session_duration_seconds: Option<i32>,
    pub total_requests: Option<i32>,
    pub unique_pages_visited: Option<i32>,
    pub avg_time_between_requests_ms: Option<i32>,
    pub request_time_variance: Option<f32>,
    pub referrer_present: Option<bool>,
    pub has_javascript: Option<bool>,
    pub accepts_cookies: Option<bool>,
    pub viewport_width: Option<i32>,
    pub viewport_height: Option<i32>,
    pub avg_scroll_depth: Option<f32>,
    pub max_scroll_depth: Option<i32>,
    pub avg_time_on_page_ms: Option<i32>,
    pub total_clicks: Option<i32>,
    pub avg_mouse_speed: Option<f32>,
    pub mouse_movement_entropy: Option<f32>,
    pub time_pattern_regularity: Option<f32>,
    pub request_burst_count: Option<i32>,
    pub headless_indicators: Option<i32>,
    pub automation_indicators: Option<i32>,
    pub fingerprint_anomaly_score: Option<f32>,
    pub feature_vector: Option<Vec<f32>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
pub struct FeatureExtractionConfig {
    pub include_session_features: bool,
    pub include_navigation_features: bool,
    pub include_behavioral_features: bool,
    pub include_timing_features: bool,
    pub normalize_features: bool,
}

impl Default for FeatureExtractionConfig {
    fn default() -> Self {
        Self {
            include_session_features: true,
            include_navigation_features: true,
            include_behavioral_features: true,
            include_timing_features: true,
            normalize_features: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AnomalyThreshold {
    pub metric_name: String,
    pub warning_threshold: f32,
    pub critical_threshold: f32,
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
}
