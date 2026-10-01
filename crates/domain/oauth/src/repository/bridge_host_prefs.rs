//! Persistence for per-user, per-host bridge preferences.
//!
//! Two independent preferences are stored:
//!
//! - `bridge_user_host_prefs`: whether a user has enabled the bridge for a
//!   host. The bridge GUI reads these at sync time so disabling a host on one
//!   device disables sync to it everywhere. "No rows at all" means every host
//!   is enabled, so this table must never gain incidental rows.
//! - `bridge_user_host_model_prefs`: an optional per-host wire-protocol filter.
//!   A row's presence is the override (an empty `model_protocols` array means
//!   "all models"); absence means the host's built-in default applies. Kept in
//!   a separate table precisely so a model-filter override never perturbs the
//!   enable-state "no rows means all" heuristic above.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_traits::RepositoryError;

use crate::error::{OauthError, OauthResult};

fn decode_host(column: &'static str, raw: &str) -> OauthResult<HostKind> {
    raw.parse::<HostKind>()
        .map_err(|source| OauthError::Repository(RepositoryError::decode(column, source)))
}

#[derive(Clone, Debug)]
pub struct BridgeHostPrefsRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl BridgeHostPrefsRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pool: db.pool(),
            write_pool: db.write_pool(),
        }
    }

    pub async fn list_enabled(&self, user_id: &UserId) -> OauthResult<Vec<HostKind>> {
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
        rows.iter()
            .map(|r| decode_host("bridge_user_host_prefs.host_id", &r.host_id))
            .collect()
    }

    pub async fn upsert(&self, user_id: &UserId, host: HostKind, enabled: bool) -> OauthResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO bridge_user_host_prefs (user_id, host_id, enabled, updated_at)
            VALUES ($1, $2, $3, CURRENT_TIMESTAMP)
            ON CONFLICT (user_id, host_id)
            DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = CURRENT_TIMESTAMP
            "#,
            user_id.as_str(),
            host.as_str(),
            enabled,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn load_model_protocols(
        &self,
        user_id: &UserId,
    ) -> OauthResult<Vec<(HostKind, Vec<String>)>> {
        let rows = sqlx::query!(
            r#"
            SELECT host_id, model_protocols FROM bridge_user_host_model_prefs
            WHERE user_id = $1
            ORDER BY host_id
            "#,
            user_id.as_str(),
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|r| {
                let host = decode_host("bridge_user_host_model_prefs.host_id", &r.host_id)?;
                Ok((host, r.model_protocols))
            })
            .collect()
    }

    pub async fn set_model_protocols(
        &self,
        user_id: &UserId,
        host: HostKind,
        protocols: Option<&[String]>,
    ) -> OauthResult<()> {
        match protocols {
            Some(list) => {
                sqlx::query!(
                    r#"
                    INSERT INTO bridge_user_host_model_prefs
                        (user_id, host_id, model_protocols, updated_at)
                    VALUES ($1, $2, $3, CURRENT_TIMESTAMP)
                    ON CONFLICT (user_id, host_id)
                    DO UPDATE SET model_protocols = EXCLUDED.model_protocols,
                                  updated_at = CURRENT_TIMESTAMP
                    "#,
                    user_id.as_str(),
                    host.as_str(),
                    list,
                )
                .execute(self.write_pool.as_ref())
                .await?;
            },
            None => {
                sqlx::query!(
                    r#"
                    DELETE FROM bridge_user_host_model_prefs
                    WHERE user_id = $1 AND host_id = $2
                    "#,
                    user_id.as_str(),
                    host.as_str(),
                )
                .execute(self.write_pool.as_ref())
                .await?;
            },
        }
        Ok(())
    }
}
