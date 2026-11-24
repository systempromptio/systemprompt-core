//! Release order of the schema install.
//!
//! The install records the release that last completed it under the release
//! stamp [`SCHEMA_INSTALL_STAMP`] (`release_stamps`), read and written under
//! the bootstrap lock. A binary older than the recorded release never applies
//! any schema statement — DDL, routines, views, triggers, migrations, seeds,
//! ownership records — so a node of release N restarting after N+1 installed
//! cannot rewrite N+1's routine bodies, views or triggers. It reads the
//! schema instead: when everything this release reads is present (the ledger
//! is current or ahead, nothing fresh, no `@compat: breaking` migration ahead)
//! the install returns a report whose `superseded` names both releases and the
//! node boots without installing; otherwise it refuses with
//! [`LoaderError::SchemaInstallSuperseded`]. An equal release runs the install
//! (the settled path applies no DDL when nothing changed); a newer release
//! installs and records itself. A release is recorded only by an install that
//! held no member back, and a database without `release_stamps` (a
//! composition without this crate's own schema) records nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::{ExtensionId, JobName};
use systemprompt_traits::RepositoryError;
use tracing::warn;

use super::super::report::SupersededInstall;
use super::current::SchemaCurrency;
use crate::repository::release_stamp::{recorded_release, release_key, stamp_release};
use crate::services::DatabaseProvider;

pub const SCHEMA_INSTALL_STAMP: &str = "schema_install";

pub const SCHEMA_INSTALL_RELEASE: &str = env!("CARGO_PKG_VERSION");

const UNDEFINED_TABLE: &str = "42P01";

pub(super) async fn newer_recorded(
    db: &dyn DatabaseProvider,
) -> Result<Option<SupersededInstall>, LoaderError> {
    let pool = db.get_postgres_pool();
    let recorded = match recorded_release(&pool, &stamp_name()).await {
        Ok(recorded) => recorded,
        Err(e) if e.sqlstate() == Some(UNDEFINED_TABLE) => None,
        Err(e) => return Err(step_failed("Failed to read the schema install release", e)),
    };
    Ok(recorded
        .filter(|recorded| release_key(recorded) > release_key(SCHEMA_INSTALL_RELEASE))
        .map(|recorded| SupersededInstall {
            release: SCHEMA_INSTALL_RELEASE.to_owned(),
            recorded,
        }))
}

pub(crate) async fn refuse_superseded(
    db: &dyn DatabaseProvider,
    work: &str,
) -> Result<(), LoaderError> {
    match newer_recorded(db).await? {
        Some(superseded) => Err(LoaderError::SchemaInstallSuperseded {
            release: superseded.release,
            recorded: superseded.recorded,
            unmet: vec![work.to_owned()],
        }),
        None => Ok(()),
    }
}

pub(super) fn judge_superseded(
    superseded: &SupersededInstall,
    currency: &SchemaCurrency,
) -> Result<(), LoaderError> {
    let mut unmet: Vec<String> = currency
        .fresh_extensions
        .iter()
        .map(|id| format!("{id}: not installed"))
        .collect();
    unmet.extend(
        currency
            .pending
            .iter()
            .map(|m| format!("{}:{:03} {} pending", m.extension_id, m.version, m.name)),
    );
    unmet.extend(
        currency
            .drift
            .iter()
            .map(|d| format!("{}:{:03} {} drifted", d.extension_id, d.version, d.name)),
    );
    unmet.extend(
        currency
            .breaking
            .iter()
            .map(|m| format!("{}:{:03} {} breaking", m.extension_id, m.version, m.name)),
    );
    if !unmet.is_empty() {
        return Err(LoaderError::SchemaInstallSuperseded {
            release: superseded.release.clone(),
            recorded: superseded.recorded.clone(),
            unmet,
        });
    }
    warn!(
        release = %superseded.release,
        recorded = %superseded.recorded,
        "Schema installed by a newer release; this release applies nothing and reads it as is"
    );
    Ok(())
}

pub(super) async fn record(db: &dyn DatabaseProvider) -> Result<(), LoaderError> {
    match stamp_release(
        &db.get_postgres_pool(),
        &stamp_name(),
        SCHEMA_INSTALL_RELEASE,
    )
    .await
    {
        Err(e) if e.sqlstate() != Some(UNDEFINED_TABLE) => Err(step_failed(
            "Failed to record the schema install release",
            e,
        )),
        _ => Ok(()),
    }
}

fn stamp_name() -> JobName {
    JobName::new(SCHEMA_INSTALL_STAMP)
}

fn step_failed(context: &str, source: RepositoryError) -> LoaderError {
    LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("database"),
        context: context.to_owned(),
        source: Box::new(source),
    }
}
