//! AI request DTOs: filters, list views, detail rows, aggregate stats, and
//! conversation messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, TraceId, UserId};

#[derive(Debug, Clone)]
pub struct AiRequestFilter {
    pub limit: i64,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub before: Option<RequestCursor>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub user: Option<String>,
}

impl AiRequestFilter {
    pub const fn new(limit: i64) -> Self {
        Self {
            limit,
            since: None,
            until: None,
            before: None,
            model: None,
            provider: None,
            user: None,
        }
    }

    pub const fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    pub const fn with_until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    pub fn with_before(mut self, before: RequestCursor) -> Self {
        self.before = Some(before);
        self
    }

    systemprompt_models::builder_methods! {
        with_model(model) -> String,
        with_provider(provider) -> String,
        with_user(user) -> String,
    }
}

/// Keyset position for paging `list_ai_requests` past its newest-first page:
/// rows strictly older than `(created_at, id)` in the list's own sort order.
///
/// The wire form is `<created_at RFC3339>@<request_id>`, which a caller
/// derives from the last row of the page it just received. `@` because the
/// remote CLI gateway refuses shell metacharacters (`|`, `;`, `&`, …) in
/// arguments, and a cursor has to survive that path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestCursor {
    pub created_at: DateTime<Utc>,
    pub id: AiRequestId,
}

impl RequestCursor {
    pub const SEPARATOR: char = '@';
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestCursorError {
    #[error("cursor is missing the `@` between the timestamp and the request id")]
    MissingSeparator,
    #[error("cursor has an empty request id")]
    EmptyId,
    #[error("cursor timestamp `{stamp}` is not RFC 3339: {source}")]
    InvalidTimestamp {
        stamp: String,
        source: chrono::ParseError,
    },
}

impl std::str::FromStr for RequestCursor {
    type Err = RequestCursorError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let (stamp, id) = raw
            .trim()
            .split_once(Self::SEPARATOR)
            .ok_or(RequestCursorError::MissingSeparator)?;
        if id.is_empty() {
            return Err(RequestCursorError::EmptyId);
        }
        let created_at = DateTime::parse_from_rfc3339(stamp)
            .map_err(|source| RequestCursorError::InvalidTimestamp {
                stamp: stamp.to_owned(),
                source,
            })?
            .with_timezone(&Utc);
        Ok(Self {
            created_at,
            id: AiRequestId::new(id),
        })
    }
}

impl std::fmt::Display for RequestCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            Self::SEPARATOR,
            self.id.as_str()
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestListItem {
    pub id: AiRequestId,
    pub created_at: DateTime<Utc>,
    pub trace_id: Option<TraceId>,
    pub user_id: UserId,
    pub actor_kind: String,
    pub actor_id: String,
    pub client_kind: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestDetail {
    pub id: AiRequestId,
    pub user_id: UserId,
    pub actor_kind: String,
    pub actor_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiRequestStats {
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
    pub by_provider: Vec<ProviderStatsRow>,
    pub by_model: Vec<ModelStatsRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatsRow {
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatsRow {
    pub model: String,
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestInfo {
    pub id: AiRequestId,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
}

/// A slice of a request's audit rows: `offset` rows in, at most `limit` rows,
/// in sequence order. A `limit` of zero means "all".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditPage {
    pub offset: i64,
    pub limit: i64,
}

impl AuditPage {
    pub const ALL: Self = Self {
        offset: 0,
        limit: 0,
    };

    pub const fn sql_limit(self) -> Option<i64> {
        if self.limit > 0 {
            Some(self.limit)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub sequence_number: i32,
}
