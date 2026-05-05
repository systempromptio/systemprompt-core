use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ContentId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsEventType {
    PageView,
    PageExit,
    LinkClick,
    Scroll,
    Engagement,
    Conversion,
    #[serde(untagged)]
    Custom(String),
}

impl AnalyticsEventType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::PageView => "page_view",
            Self::PageExit => "page_exit",
            Self::LinkClick => "link_click",
            Self::Scroll => "scroll",
            Self::Engagement => "engagement",
            Self::Conversion => "conversion",
            Self::Custom(s) => s.as_str(),
        }
    }

    pub const fn category(&self) -> &str {
        match self {
            Self::PageView | Self::PageExit => "navigation",
            Self::LinkClick => "interaction",
            Self::Scroll | Self::Engagement => "engagement",
            Self::Conversion => "conversion",
            Self::Custom(_) => "custom",
        }
    }
}

impl std::fmt::Display for AnalyticsEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAnalyticsEventInput {
    pub event_type: AnalyticsEventType,
    pub page_url: String,
    #[serde(default)]
    pub content_id: Option<ContentId>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub referrer: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAnalyticsEventBatchInput {
    pub events: Vec<CreateAnalyticsEventInput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsEventCreated {
    pub id: String,
    pub event_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsEventBatchResponse {
    pub recorded: usize,
    pub events: Vec<AnalyticsEventCreated>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngagementEventData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_scroll_depth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_on_page_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_interaction_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_scroll_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_velocity_avg: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_direction_changes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_move_distance_px: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard_events: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_events: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_time_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blur_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_switches: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_time_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden_time_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_rage_click: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dead_click: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading_pattern: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinkClickEventData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_external: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScrollEventData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub velocity: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversionEventData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funnel_step: Option<i32>,
}
