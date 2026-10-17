//! Boot-time schema currency: refuse a database the migration step has not
//! reached, and recognise one a newer release has already moved past.
//!
//! When a node boots without migrating, [`assert_schema_current`] runs while
//! the application context is built, before the domain services start, and
//! fails with the pending work and the migrate command, instead of letting
//! the first query against a missing column fail mid-request.
//!
//! A database *ahead* of the binary — ledger rows numbered past every
//! migration it declares, written by a newer release during a rolling update —
//! is not a failure. Under the expand/contract contract the previous release
//! keeps serving it; the state is logged at `WARN` and carried on the
//! [`systemprompt_database::SchemaInstallReport`] so `/readyz` can report it.
//! A booting install against an ahead database applies nothing (the newer
//! release owns that schema) and reports the same state through
//! [`warn_ahead`]. The exception is an ahead migration recorded as
//! `@compat: breaking`: [`refuse_breaking`] fails the boot, because the newer
//! release has contracted the schema this one reads.
//!
//! A running node learns the same thing later: [`SchemaState`] holds the boot
//! report and the latest ahead set, which [`recheck`] refreshes against the
//! live ledger, so a newer release's migration applied after this node booted
//! reaches `/readyz` — and an ahead `@compat: breaking` migration makes the
//! node unready instead of serving a schema it can no longer read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{PoisonError, RwLock};

use systemprompt_database::{
    DatabaseProvider, SchemaAhead, SchemaCurrency, SchemaInstallReport, schema_currency,
};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_manifest::Profile;

use crate::error::{RuntimeError, RuntimeResult};

pub async fn assert_schema_current(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    profile: &Profile,
) -> RuntimeResult<Vec<SchemaAhead>> {
    let currency = schema_currency(db, registry, &[]).await?;
    if currency.is_current() {
        let ahead = report_ahead(&currency);
        refuse_breaking(&ahead, profile)?;
        return Ok(ahead);
    }
    Err(RuntimeError::SchemaBehind {
        profile: profile.name.clone(),
        fresh: currency
            .fresh_extensions
            .iter()
            .map(ToString::to_string)
            .collect(),
        pending: currency
            .pending
            .iter()
            .map(|m| format!("{}:{:03} {}", m.extension_id, m.version, m.name))
            .collect(),
        drift: currency
            .drift
            .iter()
            .map(|d| format!("{}:{:03} {}", d.extension_id, d.version, d.name))
            .collect(),
    })
}

pub fn warn_ahead(ahead: &[SchemaAhead]) {
    if ahead.is_empty() {
        return;
    }
    let migrations: Vec<String> = ahead
        .iter()
        .map(|m| format!("{}:{:03} {}", m.extension, m.version, m.name))
        .collect();
    tracing::warn!(
        migrations = ?migrations,
        "Database schema is ahead of this binary: a newer release applied these migrations; \
         serving under the expand/contract contract until this node is replaced"
    );
}

pub fn refuse_breaking(ahead: &[SchemaAhead], profile: &Profile) -> RuntimeResult<()> {
    let breaking: Vec<String> = ahead
        .iter()
        .filter(|m| m.breaks_older)
        .map(|m| format!("{}:{:03} {}", m.extension, m.version, m.name))
        .collect();
    if breaking.is_empty() {
        return Ok(());
    }
    Err(RuntimeError::SchemaBreaksThisRelease {
        profile: profile.name.clone(),
        migrations: breaking,
    })
}

fn report_ahead(currency: &SchemaCurrency) -> Vec<SchemaAhead> {
    let ahead = SchemaAhead::from_currency(currency);
    if !currency.superseded_drift.is_empty() {
        let superseded: Vec<String> = currency
            .superseded_drift
            .iter()
            .map(|d| format!("{}:{:03} {}", d.extension_id, d.version, d.name))
            .collect();
        tracing::warn!(
            superseded_drift = ?superseded,
            "Checksum drift in extensions a newer release migrated; left to that release"
        );
    }
    warn_ahead(&ahead);
    ahead
}

/// The boot-time schema report plus the ahead set the last [`recheck`] saw.
#[derive(Debug)]
pub struct SchemaState {
    boot: SchemaInstallReport,
    ahead: RwLock<Vec<SchemaAhead>>,
}

impl From<SchemaInstallReport> for SchemaState {
    fn from(boot: SchemaInstallReport) -> Self {
        let ahead = RwLock::new(boot.ahead.clone());
        Self { boot, ahead }
    }
}

impl SchemaState {
    #[must_use]
    pub const fn boot(&self) -> &SchemaInstallReport {
        &self.boot
    }

    #[must_use]
    pub fn ahead(&self) -> Vec<SchemaAhead> {
        self.ahead
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn breaks_this_release(&self) -> bool {
        self.ahead
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|m| m.breaks_older)
    }

    fn replace(&self, ahead: Vec<SchemaAhead>) -> Vec<SchemaAhead> {
        std::mem::replace(
            &mut *self.ahead.write().unwrap_or_else(PoisonError::into_inner),
            ahead,
        )
    }
}

pub async fn recheck(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    state: &SchemaState,
) -> RuntimeResult<()> {
    let currency = schema_currency(db, registry, &[]).await?;
    let ahead = SchemaAhead::from_currency(&currency);
    let previous = state.replace(ahead.clone());
    let newly: Vec<&SchemaAhead> = ahead.iter().filter(|m| !previous.contains(m)).collect();
    if newly.is_empty() {
        return Ok(());
    }
    let migrations: Vec<String> = newly
        .iter()
        .map(|m| format!("{}:{:03} {}", m.extension, m.version, m.name))
        .collect();
    if newly.iter().any(|m| m.breaks_older) {
        tracing::error!(
            migrations = ?migrations,
            "A newer release applied a contract migration this binary cannot serve; reporting \
             not ready until this node is replaced"
        );
    } else {
        tracing::warn!(
            migrations = ?migrations,
            "A newer release applied migrations while this node was running; serving under the \
             expand/contract contract"
        );
    }
    Ok(())
}
