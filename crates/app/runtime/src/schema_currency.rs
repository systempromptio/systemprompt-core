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
//! [`warn_ahead`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::{DatabaseProvider, SchemaAhead, SchemaCurrency, schema_currency};
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
        return Ok(report_ahead(&currency));
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

fn report_ahead(currency: &SchemaCurrency) -> Vec<SchemaAhead> {
    let ahead: Vec<SchemaAhead> = currency
        .ahead
        .iter()
        .map(|m| SchemaAhead {
            extension: m.extension_id.clone(),
            version: m.version,
            name: m.name.clone(),
        })
        .collect();
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
