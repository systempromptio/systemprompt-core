use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, EngagementEventId, SessionId};

use crate::models::{CreateEngagementEventInput, EngagementEvent};

#[derive(Clone, Debug)]
pub struct EngagementRepository {
    pool: Arc<PgPool>,
}

impl EngagementRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    #[allow(clippy::cognitive_complexity)]
    pub async fn create_engagement(
        &self,
        session_id: &str,
        user_id: &str,
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
                is_rage_click, is_dead_click, reading_pattern
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24
            )
            "#,
            id.as_str(),
            session_id,
            user_id,
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
            input.optional_metrics.reading_pattern
        )
        .execute(&*self.pool)
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

    pub async fn list_by_session(&self, session_id: &str) -> Result<Vec<EngagementEvent>> {
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
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
            session_id
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(events)
    }

    pub async fn list_by_user(&self, user_id: &str, limit: i64) -> Result<Vec<EngagementEvent>> {
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
            user_id,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(events)
    }

    pub async fn get_session_engagement_summary(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionEngagementSummary>> {
        let summary = sqlx::query_as!(
            SessionEngagementSummary,
            r#"
            SELECT
                session_id,
                COUNT(*)::BIGINT as page_count,
                SUM(time_on_page_ms)::BIGINT as total_time_on_page_ms,
                AVG(max_scroll_depth)::REAL as avg_scroll_depth,
                MAX(max_scroll_depth) as max_scroll_depth,
                SUM(click_count)::BIGINT as total_clicks,
                COUNT(*) FILTER (WHERE is_rage_click = true)::BIGINT as rage_click_pages,
                MIN(created_at) as first_engagement,
                MAX(created_at) as last_engagement
            FROM engagement_events
            WHERE session_id = $1
            GROUP BY session_id
            "#,
            session_id
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(summary)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionEngagementSummary {
    pub session_id: SessionId,
    pub page_count: Option<i64>,
    pub total_time_on_page_ms: Option<i64>,
    pub avg_scroll_depth: Option<f32>,
    pub max_scroll_depth: Option<i32>,
    pub total_clicks: Option<i64>,
    pub rage_click_pages: Option<i64>,
    pub first_engagement: Option<chrono::DateTime<chrono::Utc>>,
    pub last_engagement: Option<chrono::DateTime<chrono::Utc>>,
}
