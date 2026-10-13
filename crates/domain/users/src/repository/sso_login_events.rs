//! Repository for `sso_login_events` — the audit row every browser-redirect
//! SSO sign-in leaves, accepted or refused.
//!
//! Queries are built with `sqlx::query` rather than the checked macros so the
//! table needs no offline query cache entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

use crate::error::Result;

const MAX_LIST: i64 = 500;

#[derive(Debug, Clone, Copy)]
pub struct NewSsoLoginEvent<'a> {
    pub provider_id: &'a str,
    pub issuer: &'a str,
    pub outcome: &'a str,
    pub reason: Option<&'a str>,
    pub user_id: Option<&'a UserId>,
    pub email: Option<&'a str>,
    pub external_sub: Option<&'a str>,
    pub ip: Option<IpAddr>,
    pub groups_count: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct SsoLoginEvent {
    pub id: i64,
    pub occurred_at: DateTime<Utc>,
    pub provider_id: String,
    pub issuer: String,
    pub outcome: String,
    pub reason: Option<String>,
    pub user_id: Option<UserId>,
    pub email: Option<String>,
    pub external_sub: Option<String>,
    pub ip: Option<String>,
    pub groups_count: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct SsoLoginEventRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl SsoLoginEventRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pool: db.pool(),
            write_pool: db.write_pool(),
        }
    }

    pub async fn insert_sso_login_event(&self, event: &NewSsoLoginEvent<'_>) -> Result<()> {
        sqlx::query(
            "INSERT INTO sso_login_events
             (provider_id, issuer, outcome, reason, user_id, email, external_sub, ip,
              groups_count)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8::inet, $9)",
        )
        .bind(event.provider_id)
        .bind(event.issuer)
        .bind(event.outcome)
        .bind(event.reason)
        .bind(event.user_id.map(UserId::as_str))
        .bind(event.email)
        .bind(event.external_sub)
        .bind(event.ip.map(|ip| ip.to_string()))
        .bind(event.groups_count)
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn list_recent_sso_login_events(&self, limit: i64) -> Result<Vec<SsoLoginEvent>> {
        let rows = sqlx::query(
            "SELECT id, occurred_at, provider_id, issuer, outcome, reason, user_id, email,
                    external_sub, host(ip) AS ip, groups_count
               FROM sso_login_events
              ORDER BY occurred_at DESC, id DESC
              LIMIT $1",
        )
        .bind(limit.clamp(1, MAX_LIST))
        .fetch_all(&*self.pool)
        .await?;

        rows.iter()
            .map(|r| -> Result<SsoLoginEvent> {
                Ok(SsoLoginEvent {
                    id: r.try_get("id")?,
                    occurred_at: r.try_get("occurred_at")?,
                    provider_id: r.try_get("provider_id")?,
                    issuer: r.try_get("issuer")?,
                    outcome: r.try_get("outcome")?,
                    reason: r.try_get("reason")?,
                    user_id: r.try_get::<Option<String>, _>("user_id")?.map(UserId::new),
                    email: r.try_get("email")?,
                    external_sub: r.try_get("external_sub")?,
                    ip: r.try_get("ip")?,
                    groups_count: r.try_get("groups_count")?,
                })
            })
            .collect()
    }
}
