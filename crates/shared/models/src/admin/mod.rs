use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, UserId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "TRACE"),
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: UserId,
    pub name: String,
    pub email: Option<String>,
    pub active_sessions: i64,
    pub last_session_at: Option<DateTime<Utc>>,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct UserMetricsWithTrends {
    pub total_users: i64,
    pub active_users: i64,
    pub new_users_today: i64,
    pub new_users_week: i64,
    pub new_users_month: i64,
    pub users_trend_7d: f64,
    pub users_trend_30d: f64,
    pub active_trend_7d: f64,
    pub active_trend_30d: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentStat {
    pub content_type: String,
    pub count: i64,
    pub total_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentConversation {
    pub context_id: ContextId,
    pub user_name: Option<String>,
    pub message_count: i64,
    pub last_activity: DateTime<Utc>,
    pub agent_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityTrend {
    pub date: String,
    pub message_count: i64,
    pub user_count: i64,
    pub task_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserBreakdown {
    pub browser: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceBreakdown {
    pub device_type: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicBreakdown {
    pub country: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BotTrafficStats {
    pub total_requests: i64,
    pub bot_requests: i64,
    pub human_requests: i64,
    pub bot_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsData {
    pub user_metrics: Option<UserMetricsWithTrends>,
    pub content_stats: Vec<ContentStat>,
    pub recent_conversations: Vec<RecentConversation>,
    pub activity_trends: Vec<ActivityTrend>,
    pub traffic: Option<TrafficData>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficData {
    pub browsers: Vec<BrowserBreakdown>,
    pub devices: Vec<DeviceBreakdown>,
    pub countries: Vec<GeographicBreakdown>,
    pub bot_traffic: BotTrafficStats,
}
