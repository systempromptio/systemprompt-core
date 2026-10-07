//! Boot-time refusal of a database the migration step has not reached.
//!
//! When a node boots without migrating, [`assert_schema_current`] runs while
//! the application context is built, before the domain services start, and
//! fails with the pending work and the migrate command, instead of letting
//! the first query against a missing column fail mid-request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::{DatabaseProvider, schema_currency};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_manifest::Profile;

use crate::error::{RuntimeError, RuntimeResult};

pub async fn assert_schema_current(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    profile: &Profile,
) -> RuntimeResult<()> {
    let currency = schema_currency(db, registry, &[]).await?;
    if currency.is_current() {
        return Ok(());
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
