//! Log search and summary DTOs (level/module rollups, time ranges, pattern
//! queries).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{LogId, TraceId};

#[derive(Debug, Clone)]
pub struct LogSearchFilter {
    pub pattern: String,
    pub limit: i64,
    pub since: Option<DateTime<Utc>>,
    pub level: Option<String>,
}

impl LogSearchFilter {
    pub const fn new(pattern: String, limit: i64) -> Self {
        Self {
            pattern,
            limit,
            since: None,
            level: None,
        }
    }

    pub const fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    systemprompt_models::builder_methods! {
        with_level(level) -> String,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSearchItem {
    pub id: LogId,
    pub trace_id: TraceId,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelCount {
    pub level: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCount {
    pub module: String,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LogTimeRange {
    pub earliest: Option<DateTime<Utc>>,
    pub latest: Option<DateTime<Utc>>,
}
