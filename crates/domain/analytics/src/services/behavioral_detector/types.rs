use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::SessionId;

#[derive(Debug, Clone)]
pub struct BehavioralAnalysisInput {
    pub session_id: SessionId,
    pub fingerprint_hash: Option<String>,
    pub user_agent: Option<String>,
    pub request_count: i64,
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub endpoints_accessed: Vec<String>,
    pub total_site_pages: i64,
    pub fingerprint_session_count: i64,
    pub request_timestamps: Vec<DateTime<Utc>>,
    pub has_javascript_events: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralAnalysisResult {
    pub score: i32,
    pub is_suspicious: bool,
    pub signals: Vec<BehavioralSignal>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralSignal {
    pub signal_type: SignalType,
    pub points: i32,
    pub details: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalType {
    HighRequestCount,
    HighPageCoverage,
    SequentialNavigation,
    MultipleFingerPrintSessions,
    RegularTiming,
    HighPagesPerMinute,
    OutdatedBrowser,
    NoJavaScriptEvents,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HighRequestCount => write!(f, "high_request_count"),
            Self::HighPageCoverage => write!(f, "high_page_coverage"),
            Self::SequentialNavigation => write!(f, "sequential_navigation"),
            Self::MultipleFingerPrintSessions => write!(f, "multiple_fingerprint_sessions"),
            Self::RegularTiming => write!(f, "regular_timing"),
            Self::HighPagesPerMinute => write!(f, "high_pages_per_minute"),
            Self::OutdatedBrowser => write!(f, "outdated_browser"),
            Self::NoJavaScriptEvents => write!(f, "no_javascript_events"),
        }
    }
}
