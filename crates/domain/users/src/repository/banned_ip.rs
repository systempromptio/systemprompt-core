use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use systemprompt_database::DbPool;

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

#[derive(Clone, Debug)]
pub struct BannedIpRepository {
    pool: Arc<PgPool>,
}

impl BannedIpRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn is_banned(&self, ip_address: &str) -> Result<bool> {
        let row = sqlx::query(
            r"
            SELECT EXISTS(
                SELECT 1 FROM banned_ips
                WHERE ip_address = $1
                  AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
            ) as exists
            ",
        )
        .bind(ip_address)
        .fetch_one(&*self.pool)
        .await?;

        Ok(row.get::<bool, _>("exists"))
    }

    pub async fn get_ban(&self, ip_address: &str) -> Result<Option<BannedIp>> {
        let row = sqlx::query_as::<_, BannedIp>(
            r"
            SELECT
                ip_address,
                reason,
                banned_at,
                expires_at,
                ban_count,
                last_offense_path,
                last_user_agent,
                is_permanent,
                source_fingerprint,
                ban_source,
                associated_session_ids
            FROM banned_ips
            WHERE ip_address = $1
              AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
            ",
        )
        .bind(ip_address)
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn ban_ip(&self, params: BanIpParams<'_>) -> Result<()> {
        let expires_at = params.duration.to_expiry();
        let is_permanent = matches!(params.duration, BanDuration::Permanent);

        sqlx::query(
            r"
            INSERT INTO banned_ips (
                ip_address, reason, expires_at, is_permanent,
                source_fingerprint, ban_source
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (ip_address) DO UPDATE SET
                reason = $2,
                expires_at = CASE
                    WHEN banned_ips.is_permanent THEN banned_ips.expires_at
                    ELSE COALESCE($3, banned_ips.expires_at)
                END,
                ban_count = banned_ips.ban_count + 1,
                is_permanent = banned_ips.is_permanent OR $4,
                source_fingerprint = COALESCE($5, banned_ips.source_fingerprint),
                ban_source = $6
            ",
        )
        .bind(params.ip_address)
        .bind(params.reason)
        .bind(expires_at)
        .bind(is_permanent)
        .bind(params.source_fingerprint)
        .bind(params.ban_source)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn ban_ip_with_metadata(&self, params: BanIpWithMetadataParams<'_>) -> Result<()> {
        let expires_at = params.duration.to_expiry();
        let is_permanent = matches!(params.duration, BanDuration::Permanent);
        let session_ids = params.session_id.map(|s| vec![s.to_string()]);

        sqlx::query(
            r"
            INSERT INTO banned_ips (
                ip_address, reason, expires_at, is_permanent,
                source_fingerprint, ban_source, last_offense_path,
                last_user_agent, associated_session_ids
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (ip_address) DO UPDATE SET
                reason = $2,
                expires_at = CASE
                    WHEN banned_ips.is_permanent THEN banned_ips.expires_at
                    ELSE COALESCE($3, banned_ips.expires_at)
                END,
                ban_count = banned_ips.ban_count + 1,
                is_permanent = banned_ips.is_permanent OR $4,
                source_fingerprint = COALESCE($5, banned_ips.source_fingerprint),
                ban_source = $6,
                last_offense_path = COALESCE($7, banned_ips.last_offense_path),
                last_user_agent = COALESCE($8, banned_ips.last_user_agent),
                associated_session_ids = CASE
                    WHEN $9 IS NOT NULL
                    THEN array_cat(COALESCE(banned_ips.associated_session_ids, '{}'::TEXT[]), $9)
                    ELSE banned_ips.associated_session_ids
                END
            ",
        )
        .bind(params.ip_address)
        .bind(params.reason)
        .bind(expires_at)
        .bind(is_permanent)
        .bind(params.source_fingerprint)
        .bind(params.ban_source)
        .bind(params.offense_path)
        .bind(params.user_agent)
        .bind(session_ids.as_deref())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn unban_ip(&self, ip_address: &str) -> Result<bool> {
        let result = sqlx::query(
            r"
            DELETE FROM banned_ips
            WHERE ip_address = $1
            ",
        )
        .bind(ip_address)
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn cleanup_expired(&self) -> Result<u64> {
        let result = sqlx::query(
            r"
            DELETE FROM banned_ips
            WHERE expires_at IS NOT NULL
              AND expires_at < CURRENT_TIMESTAMP
              AND NOT is_permanent
            ",
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn list_active_bans(&self, limit: i64) -> Result<Vec<BannedIp>> {
        let bans = sqlx::query_as::<_, BannedIp>(
            r"
            SELECT
                ip_address,
                reason,
                banned_at,
                expires_at,
                ban_count,
                last_offense_path,
                last_user_agent,
                is_permanent,
                source_fingerprint,
                ban_source,
                associated_session_ids
            FROM banned_ips
            WHERE expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP
            ORDER BY banned_at DESC
            LIMIT $1
            ",
        )
        .bind(limit)
        .fetch_all(&*self.pool)
        .await?;

        Ok(bans)
    }

    pub async fn list_bans_by_source(&self, ban_source: &str, limit: i64) -> Result<Vec<BannedIp>> {
        let bans = sqlx::query_as::<_, BannedIp>(
            r"
            SELECT
                ip_address,
                reason,
                banned_at,
                expires_at,
                ban_count,
                last_offense_path,
                last_user_agent,
                is_permanent,
                source_fingerprint,
                ban_source,
                associated_session_ids
            FROM banned_ips
            WHERE ban_source = $1
              AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
            ORDER BY banned_at DESC
            LIMIT $2
            ",
        )
        .bind(ban_source)
        .bind(limit)
        .fetch_all(&*self.pool)
        .await?;

        Ok(bans)
    }

    pub async fn list_bans_by_fingerprint(&self, fingerprint: &str) -> Result<Vec<BannedIp>> {
        let bans = sqlx::query_as::<_, BannedIp>(
            r"
            SELECT
                ip_address,
                reason,
                banned_at,
                expires_at,
                ban_count,
                last_offense_path,
                last_user_agent,
                is_permanent,
                source_fingerprint,
                ban_source,
                associated_session_ids
            FROM banned_ips
            WHERE source_fingerprint = $1
            ORDER BY banned_at DESC
            ",
        )
        .bind(fingerprint)
        .fetch_all(&*self.pool)
        .await?;

        Ok(bans)
    }

    pub async fn count_active_bans(&self) -> Result<i64> {
        let row = sqlx::query(
            r"
            SELECT COUNT(*)::BIGINT as count
            FROM banned_ips
            WHERE expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP
            ",
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }
}
