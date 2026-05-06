//! Persistence for active bridge sessions (one row per running bridge process).
//!
//! Rows are upserted by the bridge on each heartbeat tick and consumed by
//! product surfaces (CLI `admin bridge list`, dashboards) via
//! [`BridgeSessionRepository::list_active`].

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TenantId, UserId};

use crate::error::OauthResult;

#[derive(Clone, Debug)]
pub struct BridgeSessionRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

#[derive(Debug, Clone)]
pub struct UpsertBridgeSession {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    pub bridge_version: String,
    pub os: String,
    pub hostname: String,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub forwarded_total: i64,
    pub tokens_in_total: i64,
    pub tokens_out_total: i64,
}

#[derive(Debug, Clone)]
pub struct BridgeSessionRow {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    pub bridge_version: String,
    pub os: String,
    pub hostname: String,
    pub started_at: DateTime<Utc>,
    pub last_heartbeat_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub forwarded_total: i64,
    pub tokens_in_total: i64,
    pub tokens_out_total: i64,
}

impl BridgeSessionRepository {
    pub fn new(db: &DbPool) -> OauthResult<Self> {
        Ok(Self {
            pool: db.pool_arc()?,
            write_pool: db.write_pool_arc()?,
        })
    }

    pub async fn upsert(&self, params: UpsertBridgeSession) -> OauthResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO bridge_sessions (
                session_id, user_id, tenant_id, bridge_version, os, hostname,
                last_heartbeat_at, last_activity_at,
                forwarded_total, tokens_in_total, tokens_out_total
            )
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9, $10)
            ON CONFLICT (session_id) DO UPDATE SET
                last_heartbeat_at = NOW(),
                last_activity_at = COALESCE(EXCLUDED.last_activity_at, bridge_sessions.last_activity_at),
                bridge_version = EXCLUDED.bridge_version,
                os = EXCLUDED.os,
                hostname = EXCLUDED.hostname,
                tenant_id = EXCLUDED.tenant_id,
                forwarded_total = EXCLUDED.forwarded_total,
                tokens_in_total = EXCLUDED.tokens_in_total,
                tokens_out_total = EXCLUDED.tokens_out_total
            "#,
            params.session_id.as_str(),
            params.user_id.as_str(),
            params.tenant_id.as_ref().map(TenantId::as_str),
            params.bridge_version,
            params.os,
            params.hostname,
            params.last_activity_at,
            params.forwarded_total,
            params.tokens_in_total,
            params.tokens_out_total,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn list_active(&self, within: Duration) -> OauthResult<Vec<BridgeSessionRow>> {
        let cutoff_seconds = i64::try_from(within.as_secs()).unwrap_or(i64::MAX) as f64;
        let rows = sqlx::query_as!(
            BridgeSessionRowRaw,
            r#"
            SELECT session_id, user_id, tenant_id, bridge_version, os, hostname,
                   started_at, last_heartbeat_at, last_activity_at,
                   forwarded_total, tokens_in_total, tokens_out_total
            FROM bridge_sessions
            WHERE last_heartbeat_at > NOW() - make_interval(secs => $1::double precision)
            ORDER BY last_heartbeat_at DESC
            "#,
            cutoff_seconds,
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows.into_iter().map(BridgeSessionRow::from).collect())
    }

    pub async fn list_active_for_user(
        &self,
        user_id: &UserId,
        within: Duration,
    ) -> OauthResult<Vec<BridgeSessionRow>> {
        let cutoff_seconds = i64::try_from(within.as_secs()).unwrap_or(i64::MAX) as f64;
        let rows = sqlx::query_as!(
            BridgeSessionRowRaw,
            r#"
            SELECT session_id, user_id, tenant_id, bridge_version, os, hostname,
                   started_at, last_heartbeat_at, last_activity_at,
                   forwarded_total, tokens_in_total, tokens_out_total
            FROM bridge_sessions
            WHERE user_id = $1
              AND last_heartbeat_at > NOW() - make_interval(secs => $2::double precision)
            ORDER BY last_heartbeat_at DESC
            "#,
            user_id.as_str(),
            cutoff_seconds,
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows.into_iter().map(BridgeSessionRow::from).collect())
    }

    pub async fn delete_stale(&self, older_than: Duration) -> OauthResult<u64> {
        let cutoff_seconds = i64::try_from(older_than.as_secs()).unwrap_or(i64::MAX) as f64;
        let result = sqlx::query!(
            r#"
            DELETE FROM bridge_sessions
            WHERE last_heartbeat_at < NOW() - make_interval(secs => $1::double precision)
            "#,
            cutoff_seconds,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(result.rows_affected())
    }
}

#[derive(sqlx::FromRow)]
struct BridgeSessionRowRaw {
    session_id: String,
    user_id: String,
    tenant_id: Option<String>,
    bridge_version: String,
    os: String,
    hostname: String,
    started_at: DateTime<Utc>,
    last_heartbeat_at: DateTime<Utc>,
    last_activity_at: Option<DateTime<Utc>>,
    forwarded_total: i64,
    tokens_in_total: i64,
    tokens_out_total: i64,
}

impl From<BridgeSessionRowRaw> for BridgeSessionRow {
    fn from(raw: BridgeSessionRowRaw) -> Self {
        Self {
            session_id: SessionId::new(raw.session_id),
            user_id: UserId::new(raw.user_id),
            tenant_id: raw.tenant_id.map(TenantId::new),
            bridge_version: raw.bridge_version,
            os: raw.os,
            hostname: raw.hostname,
            started_at: raw.started_at,
            last_heartbeat_at: raw.last_heartbeat_at,
            last_activity_at: raw.last_activity_at,
            forwarded_total: raw.forwarded_total,
            tokens_in_total: raw.tokens_in_total,
            tokens_out_total: raw.tokens_out_total,
        }
    }
}
