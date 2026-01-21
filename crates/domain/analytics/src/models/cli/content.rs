use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::ContentId;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TopContentRow {
    pub content_id: ContentId,
    pub total_views: i64,
    pub unique_visitors: i64,
    pub avg_time_on_page_seconds: Option<f64>,
    pub trend_direction: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ContentStatsRow {
    pub total_views: i64,
    pub unique_visitors: i64,
    pub avg_time_on_page_seconds: Option<f64>,
    pub avg_scroll_depth: Option<f64>,
    pub total_clicks: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ContentTrendRow {
    pub timestamp: DateTime<Utc>,
    pub views: i64,
    pub unique_visitors: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrafficSourceRow {
    pub source: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GeoRow {
    pub country: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceRow {
    pub device: Option<String>,
    pub browser: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct BotTotalsRow {
    pub human: i64,
    pub bot: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BotTypeRow {
    pub bot_type: Option<String>,
    pub count: i64,
}
