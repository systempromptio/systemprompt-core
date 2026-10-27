//! Persistence for per-user, per-host bridge preferences.
//!
//! Two independent preferences are stored:
//!
//! - `bridge_user_host_prefs`: whether a user has enabled the bridge for a
//!   host. The bridge GUI reads these at sync time so disabling a host on one
//!   device disables sync to it everywhere. "No enabled rows" for the default
//!   hosts means every default host is enabled, so disabling one for a user
//!   with no such rows first records the instance's other default hosts as
//!   enabled, in one transaction. An opt-in host (see [`InstanceHosts`]) is
//!   enabled only by its own row and never counts towards that heuristic.
//! - `bridge_user_host_model_prefs`: an optional per-host wire-protocol filter.
//!   A row's presence is the override (an empty `model_protocols` array means
//!   "all models"); absence means the host's built-in default applies. Kept in
//!   a separate table precisely so a model-filter override never perturbs the
//!   enable-state "no rows means all" heuristic above.
//!
//! Both tables constrain `host_id` to the [`HostKind`] vocabulary; a row that
//! fails to parse is a schema and code version mismatch and is skipped with a
//! warning rather than failing the read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::{HostKind, InstanceHosts};

use crate::error::OauthResult;

/// A user's stored enable preferences.
///
/// `unknown_enabled_row` is true when an enabled row names a host this build
/// could not parse into a [`HostKind`]; it still counts as a choice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnabledHostPrefs {
    pub hosts: Vec<HostKind>,
    pub unknown_enabled_row: bool,
}

impl EnabledHostPrefs {
    /// Whether the user chose which default hosts they receive.
    #[must_use]
    pub fn chose_default_hosts(&self, instance: &InstanceHosts) -> bool {
        self.unknown_enabled_row || self.hosts.iter().any(|host| !instance.is_opt_in(*host))
    }

    #[must_use]
    pub fn admits(&self, host: HostKind, instance: &InstanceHosts) -> bool {
        if instance.is_opt_in(host) {
            return self.hosts.contains(&host);
        }
        !self.chose_default_hosts(instance) || self.hosts.contains(&host)
    }
}

/// Outcome of [`BridgeHostPrefsRepository::set_enabled`].
///
/// "No enabled rows" means every host, so a write that would leave none
/// enabled cannot be stored and is refused rather than silently enabling all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPrefWrite {
    Written,
    WouldDisableAll,
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

    pub async fn get_enabled_prefs(&self, user_id: &UserId) -> OauthResult<EnabledHostPrefs> {
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
        let hosts: Vec<HostKind> = rows
            .iter()
            .filter_map(|r| known_host(user_id, "bridge_user_host_prefs", &r.host_id))
            .collect();
        Ok(EnabledHostPrefs {
            unknown_enabled_row: hosts.len() < rows.len(),
            hosts,
        })
    }

    pub async fn set_enabled(
        &self,
        user_id: &UserId,
        host: HostKind,
        enabled: bool,
        instance: &InstanceHosts,
    ) -> OauthResult<HostPrefWrite> {
        let mut tx = self.write_pool.as_ref().begin().await?;
        let stored: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT host_id FROM bridge_user_host_prefs
            WHERE user_id = $1 AND enabled = true
            FOR UPDATE
            "#,
            user_id.as_str(),
        )
        .fetch_all(&mut *tx)
        .await?;
        let is_default_row =
            |raw: &str| raw.parse::<HostKind>().ok().is_none_or(|h| !instance.is_opt_in(h));
        let stored_defaults: Vec<&str> = stored
            .iter()
            .map(String::as_str)
            .filter(|raw| is_default_row(raw))
            .collect();
        if !enabled && !instance.is_opt_in(host) {
            let remaining: Vec<&str> = if stored_defaults.is_empty() {
                instance
                    .by_default()
                    .filter(|h| *h != host)
                    .map(HostKind::as_str)
                    .collect()
            } else {
                stored_defaults
                    .iter()
                    .copied()
                    .filter(|h| *h != host.as_str())
                    .collect()
            };
            if remaining.is_empty() {
                return Ok(HostPrefWrite::WouldDisableAll);
            }
            if stored_defaults.is_empty() {
                let keep: Vec<String> = remaining.iter().map(|h| (*h).to_owned()).collect();
                sqlx::query!(
                    r#"
                    INSERT INTO bridge_user_host_prefs (user_id, host_id, enabled, updated_at)
                    SELECT $1, host, true, CURRENT_TIMESTAMP FROM UNNEST($2::text[]) AS host
                    ON CONFLICT (user_id, host_id)
                    DO UPDATE SET enabled = true, updated_at = CURRENT_TIMESTAMP
                    "#,
                    user_id.as_str(),
                    &keep,
                )
                .execute(&mut *tx)
                .await?;
            }
        }
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
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(HostPrefWrite::Written)
    }

    pub async fn update_model_protocol_tag(&self, from: &str, to: &str) -> OauthResult<u64> {
        let done = sqlx::query!(
            r#"
            UPDATE bridge_user_host_model_prefs
            SET model_protocols = array_replace(model_protocols, $1, $2),
                updated_at = CURRENT_TIMESTAMP
            WHERE $1 = ANY(model_protocols)
            "#,
            from,
            to,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(done.rows_affected())
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
