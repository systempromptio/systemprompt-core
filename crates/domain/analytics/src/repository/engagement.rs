//! Persistence for page-level engagement telemetry.
//!
//! [`EngagementRepository`] records [`EngagementEvent`]s (scroll, click,
//! focus, and reading-pattern metrics) and reads them back by id or per
//! user. Writes go to the write pool; reads to the read pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, EngagementEventId, SessionId, UserId};

use crate::models::{CreateEngagementEventInput, EngagementEvent};

#[derive(Clone, Debug)]
pub struct EngagementRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl EngagementRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }

    pub async fn create_engagement(
        &self,
        session_id: &SessionId,
        user_id: &UserId,
        content_id: Option<&ContentId>,
        input: &CreateEngagementEventInput,
    ) -> Result<EngagementEventId> {
        let id = EngagementEventId::generate();

        sqlx::query!(
            r#"
            INSERT INTO engagement_events (
                id, session_id, user_id, page_url, content_id, event_type,
                time_on_page_ms, max_scroll_depth, click_count,
                time_to_first_interaction_ms, time_to_first_scroll_ms,
                scroll_velocity_avg, scroll_direction_changes,
                mouse_move_distance_px, keyboard_events, copy_events,
                focus_time_ms, blur_count, tab_switches, visible_time_ms, hidden_time_ms,
                is_rage_click, is_dead_click, reading_pattern, event_data
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25
            )
            "#,
            id.as_str(),
            session_id.as_str(),
            user_id.as_str(),
            input.page_url,
            content_id.map(ContentId::as_str),
            input.event_type.as_str(),
            input.time_on_page_ms,
            input.max_scroll_depth,
            input.click_count,
            input.optional_metrics.time_to_first_interaction_ms,
            input.optional_metrics.time_to_first_scroll_ms,
            input.optional_metrics.scroll_velocity_avg,
            input.optional_metrics.scroll_direction_changes,
            input.optional_metrics.mouse_move_distance_px,
            input.optional_metrics.keyboard_events,
            input.optional_metrics.copy_events,
            input.optional_metrics.focus_time_ms.unwrap_or(0),
            input.optional_metrics.blur_count.unwrap_or(0),
            input.optional_metrics.tab_switches.unwrap_or(0),
            input.optional_metrics.visible_time_ms.unwrap_or(0),
            input.optional_metrics.hidden_time_ms.unwrap_or(0),
            input.optional_metrics.is_rage_click,
            input.optional_metrics.is_dead_click,
            input.optional_metrics.reading_pattern,
            input.event_data.clone()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(id)
    }

    pub async fn find_by_id(&self, id: &EngagementEventId) -> Result<Option<EngagementEvent>> {
        let event = sqlx::query_as!(
            EngagementEvent,
            r#"
            SELECT
                id as "id: EngagementEventId", session_id, user_id, page_url,
                content_id as "content_id: ContentId",
                event_type,
                time_on_page_ms, time_to_first_interaction_ms, time_to_first_scroll_ms,
                max_scroll_depth, scroll_velocity_avg, scroll_direction_changes,
                click_count, mouse_move_distance_px, keyboard_events, copy_events,
                focus_time_ms as "focus_time_ms!",
                blur_count as "blur_count!",
                tab_switches as "tab_switches!",
                visible_time_ms as "visible_time_ms!",
                hidden_time_ms as "hidden_time_ms!",
                is_rage_click, is_dead_click, reading_pattern,
                created_at, updated_at
            FROM engagement_events
            WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(event)
    }

    pub async fn list_by_user(&self, user_id: &UserId, limit: i64) -> Result<Vec<EngagementEvent>> {
        let events = sqlx::query_as!(
            EngagementEvent,
            r#"
            SELECT
                id as "id: EngagementEventId", session_id, user_id, page_url,
                content_id as "content_id: ContentId",
                event_type,
                time_on_page_ms as "time_on_page_ms!", time_to_first_interaction_ms, time_to_first_scroll_ms,
                max_scroll_depth as "max_scroll_depth!", scroll_velocity_avg, scroll_direction_changes,
                click_count as "click_count!", mouse_move_distance_px, keyboard_events, copy_events,
                focus_time_ms as "focus_time_ms!",
                blur_count as "blur_count!",
                tab_switches as "tab_switches!",
                visible_time_ms as "visible_time_ms!",
                hidden_time_ms as "hidden_time_ms!",
                is_rage_click, is_dead_click, reading_pattern,
                created_at, updated_at
            FROM engagement_events
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            user_id.as_str(),
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(events)
    }
}
