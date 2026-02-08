use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{ContentId, EngagementEventId, SessionId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EngagementEvent {
    pub id: EngagementEventId,
    pub session_id: SessionId,
    pub user_id: UserId,
    pub page_url: String,
    pub content_id: Option<ContentId>,
    pub event_type: String,
    pub time_on_page_ms: i32,
    pub time_to_first_interaction_ms: Option<i32>,
    pub time_to_first_scroll_ms: Option<i32>,
    pub max_scroll_depth: i32,
    pub scroll_velocity_avg: Option<f32>,
    pub scroll_direction_changes: Option<i32>,
    pub click_count: i32,
    pub mouse_move_distance_px: Option<i32>,
    pub keyboard_events: Option<i32>,
    pub copy_events: Option<i32>,
    pub focus_time_ms: i32,
    pub blur_count: i32,
    pub tab_switches: i32,
    pub visible_time_ms: i32,
    pub hidden_time_ms: i32,
    pub is_rage_click: Option<bool>,
    pub is_dead_click: Option<bool>,
    pub reading_pattern: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CreateEngagementEventInput {
    #[serde(default)]
    pub page_url: String,
    #[serde(default = "default_event_type")]
    pub event_type: String,
    #[serde(default)]
    pub time_on_page_ms: i32,
    #[serde(default)]
    pub max_scroll_depth: i32,
    #[serde(default)]
    pub click_count: i32,
    #[serde(flatten)]
    pub optional_metrics: EngagementOptionalMetrics,
}

impl Default for CreateEngagementEventInput {
    fn default() -> Self {
        Self {
            page_url: String::new(),
            event_type: default_event_type(),
            time_on_page_ms: 0,
            max_scroll_depth: 0,
            click_count: 0,
            optional_metrics: EngagementOptionalMetrics::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EngagementOptionalMetrics {
    pub time_to_first_interaction_ms: Option<i32>,
    pub time_to_first_scroll_ms: Option<i32>,
    pub scroll_velocity_avg: Option<f32>,
    pub scroll_direction_changes: Option<i32>,
    pub mouse_move_distance_px: Option<i32>,
    pub keyboard_events: Option<i32>,
    pub copy_events: Option<i32>,
    pub focus_time_ms: Option<i32>,
    pub blur_count: Option<i32>,
    pub tab_switches: Option<i32>,
    pub visible_time_ms: Option<i32>,
    pub hidden_time_ms: Option<i32>,
    pub is_rage_click: Option<bool>,
    pub is_dead_click: Option<bool>,
    pub reading_pattern: Option<String>,
}

fn default_event_type() -> String {
    "page_exit".to_string()
}
