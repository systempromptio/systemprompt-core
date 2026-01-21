use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BannedIp {
    pub ip_address: String,
    pub reason: String,
    pub banned_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub ban_count: i32,
    pub last_offense_path: Option<String>,
    pub last_user_agent: Option<String>,
    pub is_permanent: bool,
    pub source_fingerprint: Option<String>,
    pub ban_source: Option<String>,
    pub associated_session_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy)]
pub enum BanDuration {
    Hours(i64),
    Days(i64),
    Permanent,
}

impl BanDuration {
    pub fn to_expiry(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Hours(h) => Some(Utc::now() + Duration::hours(h)),
            Self::Days(d) => Some(Utc::now() + Duration::days(d)),
            Self::Permanent => None,
        }
    }
}

pub struct BanIpParams<'a> {
    pub ip_address: &'a str,
    pub reason: &'a str,
    pub duration: BanDuration,
    pub source_fingerprint: Option<&'a str>,
    pub ban_source: &'a str,
}

impl<'a> BanIpParams<'a> {
    pub const fn new(
        ip_address: &'a str,
        reason: &'a str,
        duration: BanDuration,
        ban_source: &'a str,
    ) -> Self {
        Self {
            ip_address,
            reason,
            duration,
            source_fingerprint: None,
            ban_source,
        }
    }

    pub const fn with_source_fingerprint(mut self, fingerprint: &'a str) -> Self {
        self.source_fingerprint = Some(fingerprint);
        self
    }
}

pub struct BanIpWithMetadataParams<'a> {
    pub ip_address: &'a str,
    pub reason: &'a str,
    pub duration: BanDuration,
    pub source_fingerprint: Option<&'a str>,
    pub ban_source: &'a str,
    pub offense_path: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub session_id: Option<&'a str>,
}

impl<'a> BanIpWithMetadataParams<'a> {
    pub const fn new(
        ip_address: &'a str,
        reason: &'a str,
        duration: BanDuration,
        ban_source: &'a str,
    ) -> Self {
        Self {
            ip_address,
            reason,
            duration,
            source_fingerprint: None,
            ban_source,
            offense_path: None,
            user_agent: None,
            session_id: None,
        }
    }

    pub const fn with_source_fingerprint(mut self, fingerprint: &'a str) -> Self {
        self.source_fingerprint = Some(fingerprint);
        self
    }

    pub const fn with_offense_path(mut self, path: &'a str) -> Self {
        self.offense_path = Some(path);
        self
    }

    pub const fn with_user_agent(mut self, agent: &'a str) -> Self {
        self.user_agent = Some(agent);
        self
    }

    pub const fn with_session_id(mut self, session_id: &'a str) -> Self {
        self.session_id = Some(session_id);
        self
    }
}
