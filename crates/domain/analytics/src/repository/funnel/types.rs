use chrono::{DateTime, Utc};
use systemprompt_identifiers::{FunnelId, FunnelProgressId, SessionId};

use crate::models::{Funnel, FunnelMatchType, FunnelProgress, FunnelStep};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FunnelRow {
    pub id: FunnelId,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FunnelRow {
    pub fn into_funnel(self) -> Funnel {
        Funnel {
            id: self.id,
            name: self.name,
            description: self.description,
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FunnelStepRow {
    pub funnel_id: FunnelId,
    pub step_order: i32,
    pub name: String,
    pub match_pattern: String,
    pub match_type: String,
}

impl FunnelStepRow {
    pub fn into_step(self) -> FunnelStep {
        FunnelStep {
            funnel_id: self.funnel_id,
            step_order: self.step_order,
            name: self.name,
            match_pattern: self.match_pattern,
            match_type: FunnelMatchType::parse_type(&self.match_type),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FunnelProgressRow {
    pub id: FunnelProgressId,
    pub funnel_id: FunnelId,
    pub session_id: SessionId,
    pub current_step: i32,
    pub completed_at: Option<DateTime<Utc>>,
    pub dropped_at_step: Option<i32>,
    pub step_timestamps: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FunnelProgressRow {
    pub fn into_progress(self) -> FunnelProgress {
        FunnelProgress {
            id: self.id,
            funnel_id: self.funnel_id,
            session_id: self.session_id,
            current_step: self.current_step,
            completed_at: self.completed_at,
            dropped_at_step: self.dropped_at_step,
            step_timestamps: self.step_timestamps,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl FunnelMatchType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::UrlExact => "url_exact",
            Self::UrlPrefix => "url_prefix",
            Self::UrlRegex => "url_regex",
            Self::EventType => "event_type",
        }
    }

    pub fn parse_type(s: &str) -> Self {
        match s {
            "url_exact" => Self::UrlExact,
            "url_regex" => Self::UrlRegex,
            "event_type" => Self::EventType,
            _ => Self::UrlPrefix,
        }
    }
}
