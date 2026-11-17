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
//! Each record carries its owner's standing. An install member that claims a
//! table recorded under core standing is refused even when its id matches
//! ([`LoaderError::CoreRecordClaimedByInstall`]), so an install that takes
//! the id of a core member absent from this build adopts nothing. Records
//! are keyed by id: an install that renames its own id is refused against
//! every table recorded to the old id ([`LoaderError::DuplicateTableOwner`]
//! names that recorded owner) until an operator re-assigns the records.
//!
//! Before any DDL the install also refuses a pending migration that changes
//! a table or object another member owns, judged over the same records
//! (`migration_owners`).
//!
//! A record is released only by its owner: an install in which the owner is
//! admitted, no longer declares the table, and the table is gone (its own
//! migration chain dropped it). A database installed before the records
//! existed is recorded at its first install over the admitted members; a
//! table no admitted member declares at that point stays unrecorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_extension::{LoaderError, Migration, StandingTag};
use systemprompt_identifiers::ExtensionId;

use super::super::migration_owners::{MigrationMembers, RecordedTable, check_migration_ownership};
use super::super::prepare::PreparedSchema;
use super::super::undeclared::qualify;
use crate::lifecycle::migrations::MigrationService;
use crate::services::DatabaseProvider;

const OWNERS_PRESENT_SQL: &str = "SELECT to_regclass('schema_table_owners') IS NOT NULL AS present";

const OWNERS_SQL: &str = "SELECT table_name, extension_id, standing FROM schema_table_owners";

const RECORD_SQL: &str = "INSERT INTO schema_table_owners (table_name, extension_id, standing) \
     SELECT t.name, $2, $3 FROM unnest($1::text[]) AS t(name) \
     WHERE to_regclass(t.name) IS NOT NULL \
     ON CONFLICT (table_name) DO NOTHING";

const RELEASE_SQL: &str = "DELETE FROM schema_table_owners \
     WHERE extension_id = $1 AND NOT (table_name = ANY($2::text[])) \
     AND to_regclass(table_name) IS NULL";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TableOwners(BTreeMap<String, RecordedOwner>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedOwner {
    pub(crate) id: ExtensionId,
    pub(crate) standing: StandingTag,
}

impl TableOwners {
    #[must_use]
    pub(crate) fn owner_of(&self, table: &str) -> Option<&ExtensionId> {
        self.0.get(&qualify(table)).map(|owner| &owner.id)
    }

    #[must_use]
    pub(crate) fn is_owner(&self, extension: &ExtensionId) -> bool {
        self.0.values().any(|owner| owner.id == *extension)
    }

    fn record_of(&self, table: &str) -> Option<&RecordedOwner> {
        self.0.get(&qualify(table))
    }

    fn recorded(&self) -> Vec<RecordedTable> {
        self.0
            .iter()
            .map(|(table, owner)| RecordedTable {
                table: table.clone(),
                owner: owner.id.clone(),
                standing: owner.standing,
            })
            .collect()
    }
}

pub(super) async fn check_pending_migration_owners(
    owners: &TableOwners,
    migration_service: &MigrationService<'_>,
    judged: MigrationMembers<'_>,
    prepared: &[PreparedSchema],
) -> Result<(), LoaderError> {
    let members = judged.judged;
    let fresh = super::fresh_extension_ids(migration_service, members, prepared).await?;
    let mut applied: BTreeMap<ExtensionId, BTreeSet<u32>> = BTreeMap::new();
    for member in members
        .iter()
        .filter(|member| member.has_migrations() && !fresh.contains(member.id()))
    {
        let id = ExtensionId::new(member.id());
        let versions = migration_service
            .get_applied_migrations(&id)
            .await?
            .into_iter()
            .map(|migration| migration.version)
            .collect();
        applied.insert(id, versions);
    }
    let pending = |id: &ExtensionId, migration: &Migration| {
        applied
            .get(id)
            .is_some_and(|versions| !versions.contains(&migration.version))
    };
    check_migration_ownership(judged, &owners.recorded(), &pending).map_err(|error| {
        LoaderError::SchemaInstallationStepFailed {
            extension: error.member().clone(),
            context: "a pending migration is refused by the migration ownership rule".to_owned(),
            source: Box::new(error),
        }
    })
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
                let standing = StandingTag::parse(row.get("standing")?.as_str()?)?;
                Some((
                    qualify(table),
                    RecordedOwner {
                        id: ExtensionId::new(owner),
                        standing,
                    },
                ))
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
            let Some(recorded) = owners.record_of(table) else {
                continue;
            };
            if recorded.id != p.extension_id {
                return Err(LoaderError::DuplicateTableOwner {
                    table: table.clone(),
                    extension_a: recorded.id.clone(),
                    extension_b: p.extension_id.clone(),
                });
            }
            if recorded.standing == StandingTag::Core && p.standing == StandingTag::Install {
                return Err(LoaderError::CoreRecordClaimedByInstall {
                    kind: "table",
                    name: table.clone(),
                    owner: p.extension_id.clone(),
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
            db.execute(
                &RECORD_SQL,
                &[&declared, &p.extension_id.as_str(), &p.standing.as_str()],
            )
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
