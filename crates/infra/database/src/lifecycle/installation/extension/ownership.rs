//! Persisted table ownership: the member that created each table.
//!
//! The installer sees only the admitted composition, and a disabled member's
//! code never runs, so its declarations cannot be re-read at boot. Each
//! install therefore records, per table an admitted member declares and the
//! database holds, that member's id in `schema_table_owners`. Before any DDL
//! the install refuses a member that declares a table recorded to a different
//! member, admitted or not, so a peer's `CREATE TABLE IF NOT EXISTS` can never
//! adopt a disabled member's table and its rows. The residue audit reads the
//! same records to tell a table owned by a member outside this composition
//! from an undeclared one.
//!
//! A record is released only by its owner: an install in which the owner is
//! admitted, no longer declares the table, and the table is gone (its own
//! migration chain dropped it). A database installed before the records
//! existed is recorded at its first install over the admitted members; a
//! table no admitted member declares at that point stays unrecorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;

use super::super::prepare::PreparedSchema;
use super::super::undeclared::qualify;
use crate::services::DatabaseProvider;

const OWNERS_PRESENT_SQL: &str = "SELECT to_regclass('schema_table_owners') IS NOT NULL AS present";

const OWNERS_SQL: &str = "SELECT table_name, extension_id FROM schema_table_owners";

const RECORD_SQL: &str = "INSERT INTO schema_table_owners (table_name, extension_id) \
     SELECT t.name, $2 FROM unnest($1::text[]) AS t(name) \
     WHERE to_regclass(t.name) IS NOT NULL \
     ON CONFLICT (table_name) DO NOTHING";

const RELEASE_SQL: &str = "DELETE FROM schema_table_owners \
     WHERE extension_id = $1 AND NOT (table_name = ANY($2::text[])) \
     AND to_regclass(table_name) IS NULL";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TableOwners(BTreeMap<String, ExtensionId>);

impl TableOwners {
    #[must_use]
    pub(crate) fn owner_of(&self, table: &str) -> Option<&ExtensionId> {
        self.0.get(&qualify(table))
    }

    #[must_use]
    pub(crate) fn is_owner(&self, extension: &ExtensionId) -> bool {
        self.0.values().any(|owner| owner == extension)
    }
}

pub(crate) async fn recorded_table_owners(
    db: &dyn DatabaseProvider,
) -> Result<TableOwners, LoaderError> {
    if !recorded_table_owners_present(db).await? {
        return Ok(TableOwners::default());
    }
    let rows = db
        .fetch_all(&OWNERS_SQL, &[])
        .await
        .map_err(|e| step_failed("Failed to read the table ownership records", e))?;
    Ok(TableOwners(
        rows.iter()
            .filter_map(|row| {
                let table = row.get("table_name")?.as_str()?;
                let owner = row.get("extension_id")?.as_str()?;
                Some((qualify(table), ExtensionId::new(owner)))
            })
            .collect(),
    ))
}

pub(super) fn check_recorded_owners(
    owners: &TableOwners,
    prepared: &[PreparedSchema],
) -> Result<(), LoaderError> {
    for p in prepared {
        for table in &p.owned_tables {
            if let Some(recorded) = owners.owner_of(table)
                && *recorded != p.extension_id
            {
                return Err(LoaderError::DuplicateTableOwner {
                    table: table.clone(),
                    extension_a: recorded.clone(),
                    extension_b: p.extension_id.clone(),
                });
            }
        }
    }
    Ok(())
}

pub(super) async fn record_owners(
    db: &dyn DatabaseProvider,
    prepared: &[PreparedSchema],
) -> Result<(), LoaderError> {
    if recorded_table_owners_present(db).await? {
        for p in prepared {
            let declared: Vec<String> = p.owned_tables.iter().map(|t| qualify(t)).collect();
            db.execute(&RELEASE_SQL, &[&p.extension_id.as_str(), &declared])
                .await
                .map_err(|e| step_failed("Failed to release dropped table ownership", e))?;
            db.execute(&RECORD_SQL, &[&declared, &p.extension_id.as_str()])
                .await
                .map_err(|e| step_failed("Failed to record table ownership", e))?;
        }
    }
    Ok(())
}

async fn recorded_table_owners_present(db: &dyn DatabaseProvider) -> Result<bool, LoaderError> {
    let row = db
        .fetch_one(&OWNERS_PRESENT_SQL, &[])
        .await
        .map_err(|e| step_failed("Failed to look up the table ownership records", e))?;
    Ok(row
        .get("present")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false))
}

fn step_failed(context: &str, source: crate::error::RepositoryError) -> LoaderError {
    LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("database"),
        context: context.to_owned(),
        source: Box::new(source),
    }
}
