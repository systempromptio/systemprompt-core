use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GenerateLinkRequest {
    pub target_url: String,
    pub link_type: String,
    pub campaign_id: Option<String>,
    pub campaign_name: Option<String>,
    pub source_content_id: Option<String>,
    pub source_page: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
    pub link_text: Option<String>,
    pub link_position: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct GenerateLinkResponse {
    pub link_id: String,
    pub short_code: String,
    pub redirect_url: String,
    pub full_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ListLinksQuery {
    pub campaign_id: Option<String>,
    pub source_content_id: Option<String>,
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub struct AnalyticsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn internal_error(message: &str) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": message})),
    )
}
