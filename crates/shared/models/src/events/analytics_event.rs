use axum::response::sse::Event;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnalyticsEvent {
    SessionStarted {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: SessionStartedPayload,
    },
    SessionEnded {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: SessionEndedPayload,
    },
    PageView {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: PageViewPayload,
    },
    EngagementUpdate {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: EngagementUpdatePayload,
    },
    RealTimeStats {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: RealTimeStatsPayload,
    },
    Heartbeat {
        timestamp: DateTime<Utc>,
    },
}

impl AnalyticsEvent {
    pub const fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::SessionStarted { timestamp, .. }
            | Self::SessionEnded { timestamp, .. }
            | Self::PageView { timestamp, .. }
            | Self::EngagementUpdate { timestamp, .. }
            | Self::RealTimeStats { timestamp, .. }
            | Self::Heartbeat { timestamp } => *timestamp,
        }
    }

    pub fn to_sse(&self) -> Result<Event, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(Event::default().data(json))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartedPayload {
    pub session_id: String,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub country: Option<String>,
    pub referrer_source: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndedPayload {
    pub session_id: String,
    pub duration_ms: i64,
    pub page_count: i64,
    pub request_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageViewPayload {
    pub session_id: String,
    pub user_id: Option<String>,
    pub page_url: String,
    pub content_id: Option<String>,
    pub referrer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementUpdatePayload {
    pub session_id: String,
    pub page_url: String,
    pub scroll_depth: i32,
    pub time_on_page_ms: i64,
    pub click_count: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RealTimeStatsPayload {
    pub active_sessions: i64,
    pub active_users: i64,
    pub requests_per_minute: i64,
    pub page_views_last_5m: i64,
    pub bot_requests_last_5m: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct AnalyticsEventBuilder;

impl AnalyticsEventBuilder {
    pub fn session_started(
        session_id: String,
        device_type: Option<String>,
        browser: Option<String>,
        os: Option<String>,
        country: Option<String>,
        referrer_source: Option<String>,
        is_bot: bool,
    ) -> AnalyticsEvent {
        AnalyticsEvent::SessionStarted {
            timestamp: Utc::now(),
            payload: SessionStartedPayload {
                session_id,
                device_type,
                browser,
                os,
                country,
                referrer_source,
                is_bot,
            },
        }
    }

    pub fn session_ended(
        session_id: String,
        duration_ms: i64,
        page_count: i64,
        request_count: i64,
    ) -> AnalyticsEvent {
        AnalyticsEvent::SessionEnded {
            timestamp: Utc::now(),
            payload: SessionEndedPayload {
                session_id,
                duration_ms,
                page_count,
                request_count,
            },
        }
    }

    pub fn page_view(
        session_id: String,
        user_id: Option<String>,
        page_url: String,
        content_id: Option<String>,
        referrer: Option<String>,
    ) -> AnalyticsEvent {
        AnalyticsEvent::PageView {
            timestamp: Utc::now(),
            payload: PageViewPayload {
                session_id,
                user_id,
                page_url,
                content_id,
                referrer,
            },
        }
    }

    pub fn engagement_update(
        session_id: String,
        page_url: String,
        scroll_depth: i32,
        time_on_page_ms: i64,
        click_count: i32,
    ) -> AnalyticsEvent {
        AnalyticsEvent::EngagementUpdate {
            timestamp: Utc::now(),
            payload: EngagementUpdatePayload {
                session_id,
                page_url,
                scroll_depth,
                time_on_page_ms,
                click_count,
            },
        }
    }

    pub fn realtime_stats(
        active_sessions: i64,
        active_users: i64,
        requests_per_minute: i64,
        page_views_last_5m: i64,
        bot_requests_last_5m: i64,
    ) -> AnalyticsEvent {
        AnalyticsEvent::RealTimeStats {
            timestamp: Utc::now(),
            payload: RealTimeStatsPayload {
                active_sessions,
                active_users,
                requests_per_minute,
                page_views_last_5m,
                bot_requests_last_5m,
            },
        }
    }

    pub fn heartbeat() -> AnalyticsEvent {
        AnalyticsEvent::Heartbeat {
            timestamp: Utc::now(),
        }
    }
}
