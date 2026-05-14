//! Persistence for per-user, per-host bridge enable/disable preferences.
//!
//! Each `(user_id, host_id)` row records whether that user has enabled the
//! bridge for a given host. The bridge GUI reads these prefs at sync time so
//! that disabling a host on one device disables sync to it everywhere.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

use crate::error::OauthResult;

#[derive(Clone, Debug)]
pub struct BridgeHostPrefsRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl BridgeHostPrefsRepository {
    pub fn new(db: &DbPool) -> OauthResult<Self> {
        Ok(Self {
            pool: db.pool_arc()?,
            write_pool: db.write_pool_arc()?,
        })
    }

    pub async fn list_enabled(&self, user_id: &UserId) -> OauthResult<Vec<String>> {
        let rows = sqlx::query!(
            r#"
            SELECT host_id FROM bridge_user_host_prefs
            WHERE user_id = $1 AND enabled = true
            ORDER BY host_id
            "#,
            user_id.as_str(),
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows.into_iter().map(|r| r.host_id).collect())
    }

    pub async fn upsert(&self, user_id: &UserId, host_id: &str, enabled: bool) -> OauthResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO bridge_user_host_prefs (user_id, host_id, enabled, updated_at)
            VALUES ($1, $2, $3, CURRENT_TIMESTAMP)
            ON CONFLICT (user_id, host_id)
            DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = CURRENT_TIMESTAMP
            "#,
            user_id.as_str(),
            host_id,
            enabled,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }
}
