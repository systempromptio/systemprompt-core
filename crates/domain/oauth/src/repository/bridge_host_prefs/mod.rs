//! Persistence for per-user, per-host bridge preferences.
//!
//! Two independent preferences are stored:
//!
//! - `bridge_user_host_prefs`: whether a user has enabled a host. The rows hold
//!   two independent sets. The hosts the bridge configures
//!   ([`ConfigOwner::Bridge`]) that the instance serves by default follow "no
//!   enabled rows means every one of them": the bridge GUI reads them at sync
//!   time so disabling a host on one device disables sync to it everywhere, and
//!   disabling one for a user with no such rows first records the instance's
//!   other bridge-configured default hosts as enabled, in one transaction. A
//!   host that configures itself ([`ConfigOwner::Host`]) and an opt-in host
//!   (see [`InstanceHosts`]) are each enabled only by their own row: writing
//!   one never reads, counts towards or changes the first set, and
//!   `default_enabled` does not reach a host that configures itself. The
//!   last-host guard counts the first set only, and a stored row this build
//!   cannot parse is never the host that satisfies it.
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

use async_trait::async_trait;
use systemprompt_traits::{BridgeHostFilterMigration, RepositoryError};

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::{ConfigOwner, HostKind, InstanceHosts};

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
    #[must_use]
    pub fn chose_default_hosts(&self, instance: &InstanceHosts) -> bool {
        self.unknown_enabled_row
            || self
                .hosts
                .iter()
                .any(|host| !enabled_by_own_row(*host, instance))
    }

    #[must_use]
    pub fn admits(&self, host: HostKind, instance: &InstanceHosts) -> bool {
        if enabled_by_own_row(host, instance) {
            return self.hosts.contains(&host);
        }
        !self.chose_default_hosts(instance) || self.hosts.contains(&host)
    }
}

/// Outcome of [`BridgeHostPrefsRepository::set_enabled`].
///
/// "No enabled rows" means every host, so a write that would leave no
/// bridge-configured host enabled cannot be stored and is refused rather than
/// silently enabling all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPrefWrite {
    Written,
    WouldDisableAll,
}

fn enabled_by_own_row(host: HostKind, instance: &InstanceHosts) -> bool {
    match host.config_owner() {
        ConfigOwner::Host => true,
        ConfigOwner::Bridge => instance.is_opt_in(host),
    }
}

enum StoredRow {
    BridgeDefault(HostKind),
    OwnRow,
    Unreadable,
}

// Why: Postgres may hold a row a newer build wrote; an id this build cannot
// parse still counts as a choice, as `EnabledHostPrefs::unknown_enabled_row`
// has it, but is never a host the last-host guard may count.
fn stored_row(raw: &str, instance: &InstanceHosts) -> StoredRow {
    match raw.parse::<HostKind>() {
        Ok(host) if enabled_by_own_row(host, instance) => StoredRow::OwnRow,
        Ok(host) => StoredRow::BridgeDefault(host),
        Err(_) => StoredRow::Unreadable,
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

    pub async fn get_enabled_prefs(&self, user_id: &UserId) -> OauthResult<EnabledHostPrefs> {
        let rows = sqlx::query!(
            r#"
            SELECT host_id FROM bridge_user_host_prefs
            WHERE user_id = $1 AND enabled = true
            ORDER BY host_id
            "#,
            user_id as _,
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
        if !enabled && !enabled_by_own_row(host, instance) {
            let stored: Vec<String> = sqlx::query_scalar!(
                r#"
            SELECT host_id FROM bridge_user_host_prefs
            WHERE user_id = $1 AND enabled = true
            FOR UPDATE
            "#,
                user_id as _,
            )
            .fetch_all(&mut *tx)
            .await?;
            let rows: Vec<StoredRow> = stored.iter().map(|raw| stored_row(raw, instance)).collect();
            let chose = rows.iter().any(|row| !matches!(row, StoredRow::OwnRow));
            let remaining: Vec<HostKind> = if chose {
                rows.iter()
                    .filter_map(|row| match row {
                        StoredRow::BridgeDefault(kept) if *kept != host => Some(*kept),
                        StoredRow::BridgeDefault(_) | StoredRow::OwnRow | StoredRow::Unreadable => {
                            None
                        },
                    })
                    .collect()
            } else {
                instance
                    .by_default()
                    .filter(|kept| *kept != host && !enabled_by_own_row(*kept, instance))
                    .collect()
            };
            if remaining.is_empty() {
                return Ok(HostPrefWrite::WouldDisableAll);
            }
            if !chose {
                let keep: Vec<String> = remaining
                    .iter()
                    .map(|kept| kept.as_str().to_owned())
                    .collect();
                sqlx::query!(
                    r#"
                    INSERT INTO bridge_user_host_prefs (user_id, host_id, enabled, updated_at)
                    SELECT $1, host, true, CURRENT_TIMESTAMP FROM UNNEST($2::text[]) AS host
                    ON CONFLICT (user_id, host_id)
                    DO UPDATE SET enabled = true, updated_at = CURRENT_TIMESTAMP
                    "#,
                    user_id as _,
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
            user_id as _,
            host.as_str(),
            enabled,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(HostPrefWrite::Written)
    }

    pub async fn update_model_protocol_tag(
        &self,
        from: &str,
        to: &str,
    ) -> Result<u64, RepositoryError> {
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
        .await
        .map_err(RepositoryError::from)?;
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
            user_id as _,
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
                    user_id as _,
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
                    user_id as _,
                    host.as_str(),
                )
                .execute(self.write_pool.as_ref())
                .await?;
            },
        }
        Ok(())
    }
}

#[async_trait]
impl BridgeHostFilterMigration for BridgeHostPrefsRepository {
    async fn rename_model_protocol_tag(
        &self,
        from: &str,
        to: &str,
    ) -> Result<u64, RepositoryError> {
        self.update_model_protocol_tag(from, to).await
    }
}
