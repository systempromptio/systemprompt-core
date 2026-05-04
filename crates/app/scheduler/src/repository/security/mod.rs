//! Security-feature queries used by the malicious-IP blacklist job.
//!
//! The repository deliberately reads from the read replica only; mutations
//! to ban tables live in `systemprompt-users`.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::error::SchedulerResult;

/// Repository surfacing IP-aggregation queries against `user_sessions`.
#[derive(Debug, Clone)]
pub struct SecurityRepository {
    pool: Arc<PgPool>,
}

/// Aggregate row describing IP activity in the last 24 hours.
#[derive(Debug)]
pub struct IpSessionRecord {
    /// Source IP address of the aggregated sessions.
    pub ip_address: Option<String>,
    /// Country code as recorded on the session, when available.
    pub country: Option<String>,
    /// Number of sessions that contributed to this aggregate.
    pub session_count: i64,
}

impl SecurityRepository {
    /// Construct a new repository from the shared [`DbPool`].
    pub fn new(db: &DbPool) -> SchedulerResult<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    /// Find IPs that ran at least `threshold` non-bot sessions in the last
    /// 24 hours.
    pub async fn find_high_volume_ips(
        &self,
        threshold: i64,
    ) -> SchedulerResult<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
              AND is_bot = false
            GROUP BY ip_address
            HAVING COUNT(*) >= $1
            "#,
            threshold
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    /// Find IPs that the analytics scanner-detector flagged at least
    /// `threshold` times in the last 24 hours.
    pub async fn find_scanner_ips(&self, threshold: i64) -> SchedulerResult<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
              AND is_scanner = true
            GROUP BY ip_address
            HAVING COUNT(*) >= $1
            "#,
            threshold
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    /// Return per-IP session aggregates for every distinct IP active in the
    /// last 24 hours.
    pub async fn find_recent_ips(&self) -> SchedulerResult<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
            GROUP BY ip_address
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    /// Find IPs from any country that contributed at least `threshold`
    /// sessions in the last 24 hours; the caller filters by high-risk
    /// country list.
    pub async fn find_high_risk_country_ips(
        &self,
        threshold: i64,
    ) -> SchedulerResult<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, country, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
              AND country IS NOT NULL
            GROUP BY ip_address, country
            HAVING COUNT(*) >= $1
            "#,
            threshold
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: r.country,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }
}
