use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

const BEHAVIORAL_BOT_SCORE_THRESHOLD: i32 = 50;
const HIGH_REQUESTS_PER_MINUTE_THRESHOLD: f64 = 30.0;
const HIGH_ERROR_RATE_THRESHOLD: f64 = 0.5;
const MIN_REQUESTS_FOR_ERROR_ESCALATION: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ThrottleLevel {
    Normal = 0,
    Warning = 1,
    Severe = 2,
    Blocked = 3,
}

impl From<i32> for ThrottleLevel {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Warning,
            2 => Self::Severe,
            3 => Self::Blocked,
            _ => Self::Normal,
        }
    }
}

impl From<ThrottleLevel> for i32 {
    fn from(level: ThrottleLevel) -> Self {
        level as i32
    }
}

impl ThrottleLevel {
    pub const fn rate_multiplier(self) -> f64 {
        match self {
            Self::Normal => 1.0,
            Self::Warning => 0.5,
            Self::Severe => 0.25,
            Self::Blocked => 0.0,
        }
    }

    pub const fn allows_requests(self) -> bool {
        !matches!(self, Self::Blocked)
    }

    pub const fn escalate(self) -> Self {
        match self {
            Self::Normal => Self::Warning,
            Self::Warning => Self::Severe,
            Self::Severe | Self::Blocked => Self::Blocked,
        }
    }

    pub const fn deescalate(self) -> Self {
        match self {
            Self::Normal | Self::Warning => Self::Normal,
            Self::Severe => Self::Warning,
            Self::Blocked => Self::Severe,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EscalationCriteria {
    pub behavioral_bot_score: i32,
    pub request_count: i64,
    pub error_rate: f64,
    pub requests_per_minute: f64,
}

impl Copy for EscalationCriteria {}

#[derive(Debug, Clone, Copy, Default)]
pub struct ThrottleService;

impl ThrottleService {
    pub const fn new() -> Self {
        Self
    }

    pub fn should_escalate(criteria: &EscalationCriteria, current_level: ThrottleLevel) -> bool {
        if current_level == ThrottleLevel::Blocked {
            return false;
        }

        if criteria.behavioral_bot_score >= BEHAVIORAL_BOT_SCORE_THRESHOLD {
            return true;
        }

        if criteria.requests_per_minute > HIGH_REQUESTS_PER_MINUTE_THRESHOLD {
            return true;
        }

        if criteria.error_rate > HIGH_ERROR_RATE_THRESHOLD
            && criteria.request_count > MIN_REQUESTS_FOR_ERROR_ESCALATION
        {
            return true;
        }

        false
    }

    pub fn adjusted_rate_limit(base_rate: u64, level: ThrottleLevel) -> u64 {
        let multiplier = level.rate_multiplier();
        ((base_rate as f64) * multiplier).max(1.0) as u64
    }

    pub fn can_deescalate(
        current_level: ThrottleLevel,
        last_escalation: Option<DateTime<Utc>>,
        cooldown_minutes: i64,
    ) -> bool {
        if current_level == ThrottleLevel::Normal {
            return false;
        }

        last_escalation.is_none_or(|escalated_at| {
            Utc::now() > escalated_at + Duration::minutes(cooldown_minutes)
        })
    }
}
