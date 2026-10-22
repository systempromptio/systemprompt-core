//! Flat `user_sessions` row decoded by the snapshot queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::session_store::{
    SessionActivityCounts, SessionAttributionSnapshot, SessionClassification,
    SessionClientSnapshot, SessionSnapshot, SessionTiming,
};

pub(super) struct SessionSnapshotRow {
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub fingerprint_hash: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub referrer_url: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_content: Option<String>,
    pub utm_term: Option<String>,
    pub is_bot: bool,
    pub is_scanner: Option<bool>,
    pub is_behavioral_bot: Option<bool>,
    pub behavioral_bot_reason: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub request_count: Option<i32>,
    pub task_count: Option<i32>,
    pub ai_request_count: Option<i32>,
    pub message_count: Option<i32>,
}

impl From<SessionSnapshotRow> for SessionSnapshot {
    fn from(row: SessionSnapshotRow) -> Self {
        Self {
            session_id: row.session_id,
            user_id: row.user_id,
            fingerprint_hash: row.fingerprint_hash,
            client: SessionClientSnapshot {
                ip_address: row.ip_address,
                user_agent: row.user_agent,
                device_type: row.device_type,
                browser: row.browser,
                os: row.os,
                country: row.country,
                city: row.city,
            },
            attribution: SessionAttributionSnapshot {
                referrer_url: row.referrer_url,
                utm_source: row.utm_source,
                utm_medium: row.utm_medium,
                utm_campaign: row.utm_campaign,
                utm_content: row.utm_content,
                utm_term: row.utm_term,
            },
            classification: SessionClassification {
                is_bot: row.is_bot,
                is_scanner: row.is_scanner,
                is_behavioral_bot: row.is_behavioral_bot,
                behavioral_bot_reason: row.behavioral_bot_reason,
            },
            timing: SessionTiming {
                started_at: row.started_at,
                last_activity_at: row.last_activity_at,
                ended_at: row.ended_at,
            },
            activity: SessionActivityCounts {
                request_count: row.request_count,
                task_count: row.task_count,
                ai_request_count: row.ai_request_count,
                message_count: row.message_count,
            },
        }
    }
}
