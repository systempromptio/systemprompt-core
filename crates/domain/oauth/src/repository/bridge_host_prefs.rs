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
//! Rows written by earlier releases may name a host that no longer exists
//! (`cowork`). Such a row is skipped with a warning rather than failing the
//! read: it selects no host the bridge can run, but it still counts as a
//! stored enable preference, so it never turns "some hosts" into "all hosts".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::HostKind;

use crate::error::OauthResult;

/// A user's stored enable preferences.
///
/// `any_enabled_row` is true when at least one enabled row exists, including
/// rows naming a host outside [`HostKind`] that were skipped on read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnabledHostPrefs {
    pub hosts: Vec<HostKind>,
    pub any_enabled_row: bool,
}

impl EnabledHostPrefs {
    #[must_use]
    pub fn admits(&self, host: HostKind) -> bool {
        !self.any_enabled_row || self.hosts.contains(&host)
    }
}

fn known_host(user_id: &UserId, table: &'static str, raw: &str) -> Option<HostKind> {
    match raw.parse::<HostKind>() {
        Ok(host) => Some(host),
        Err(error) => {
            tracing::warn!(
                user_id = %user_id,
                table,
                %error,
                "Skipping a stored bridge host preference for an unknown host"
            );
            None
        },
    }
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

    pub async fn list_enabled(&self, user_id: &UserId) -> OauthResult<EnabledHostPrefs> {
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
        Ok(EnabledHostPrefs {
            any_enabled_row: !rows.is_empty(),
            hosts: rows
                .iter()
                .filter_map(|r| known_host(user_id, "bridge_user_host_prefs", &r.host_id))
                .collect(),
        })
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
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                known_host(user_id, "bridge_user_host_model_prefs", &r.host_id)
                    .map(|host| (host, r.model_protocols))
            })
            .collect())
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
