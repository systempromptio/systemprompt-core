//! Private decode target for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so engagement rows decode `user_id` as a
//! plain string here and become a typed id through the trusted `new`
//! constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ContentId, EngagementEventId, SessionId, UserId};

use super::EngagementEvent;

#[derive(Debug)]
pub(crate) struct EngagementEventRow {
    pub id: EngagementEventId,
    pub session_id: SessionId,
    pub user_id: String,
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

impl From<EngagementEventRow> for EngagementEvent {
    fn from(row: EngagementEventRow) -> Self {
        Self {
            id: row.id,
            session_id: row.session_id,
            user_id: UserId::new(row.user_id),
            page_url: row.page_url,
            content_id: row.content_id,
            event_type: row.event_type,
            time_on_page_ms: row.time_on_page_ms,
            time_to_first_interaction_ms: row.time_to_first_interaction_ms,
            time_to_first_scroll_ms: row.time_to_first_scroll_ms,
            max_scroll_depth: row.max_scroll_depth,
            scroll_velocity_avg: row.scroll_velocity_avg,
            scroll_direction_changes: row.scroll_direction_changes,
            click_count: row.click_count,
            mouse_move_distance_px: row.mouse_move_distance_px,
            keyboard_events: row.keyboard_events,
            copy_events: row.copy_events,
            focus_time_ms: row.focus_time_ms,
            blur_count: row.blur_count,
            tab_switches: row.tab_switches,
            visible_time_ms: row.visible_time_ms,
            hidden_time_ms: row.hidden_time_ms,
            is_rage_click: row.is_rage_click,
            is_dead_click: row.is_dead_click,
            reading_pattern: row.reading_pattern,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
