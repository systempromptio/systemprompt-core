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
//! A record carrying install standing yields to a core member's SQL only
//! while its owner is admitted and judged in this install: a core member
//! declaring such a table is not refused by that record, the record is no
//! claim when the migration ownership rule ranks, the owner's own declaration
//! of the table is refused as shared, and the install deletes the record
//! after its DDL. When the recorded owner is held back or absent, the record
//! outranks every member: a core member declaring the table is refused
//! ([`LoaderError::DuplicateTableOwner`]), and the record, the table's rows
//! and the owner's purge record stay.
//!
//! Each install also records, in `schema_object_owners`, the owner of every
//! index, routine, view, materialized view, sequence, type, domain and
//! trigger an admitted member's SQL creates, and deletes stale records (see
//! `object_owners`). Before any DDL the install refuses a pending
//! migration that changes a table or object another member owns, judged over
//! both sets of records (`migration_owners`), so a disabled member keeps its
//! objects as it keeps its tables.
//!
//! A record is released only by its owner: an install in which the owner is
//! admitted and either no longer declares the table and the table is gone
//! (its own migration chain dropped it), or names the table in its
//! retirements and no longer declares it (its chain retired the table, which
//! may still exist on an established database until another member's
//! migration drops it). An admitted core owner retiring a table hands it, its
//! rows and its record to the core member declaring it in that install. A
//! database installed before the records existed is recorded at its first
//! install over the admitted members; a table no admitted member declares at
//! that point stays unrecorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::PreparedAt;
use std::collections::{BTreeMap, BTreeSet};

use systemprompt_extension::{LoaderError, Migration, StandingTag};
use systemprompt_identifiers::ExtensionId;

use super::super::migration_owners::{
    MigrationMembers, Recorded, RecordedTable, check_migration_ownership, record_plan,
};
use super::super::prepare::PreparedSchema;
use super::super::undeclared::qualify;
use super::object_owners::{record_object_owners, recorded_object_owners, release_stale_records};
use crate::lifecycle::migrations::MigrationService;
use crate::services::DatabaseProvider;

const OWNERS_PRESENT_SQL: &str = "SELECT to_regclass('schema_table_owners') IS NOT NULL AS present";

const OWNERS_SQL: &str = "SELECT table_name, extension_id, standing FROM schema_table_owners";

const RECORD_SQL: &str = "INSERT INTO schema_table_owners (table_name, extension_id, standing) \
     SELECT t.name, $2, $3 FROM unnest($1::text[]) AS t(name) \
     WHERE to_regclass(t.name) IS NOT NULL \
     ON CONFLICT (table_name) DO NOTHING";

const RELEASE_SQL: &str = "DELETE FROM schema_table_owners \
     WHERE extension_id = $1 AND (table_name = ANY($3::text[]) \
     OR (NOT (table_name = ANY($2::text[])) AND to_regclass(table_name) IS NULL))";

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

    pub(super) fn recorded(&self) -> Vec<RecordedTable> {
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
    db: &dyn DatabaseProvider,
    owners: &TableOwners,
    migration_service: &MigrationService<'_>,
    judged: MigrationMembers<'_>,
    PreparedAt { prepared, target }: PreparedAt<'_>,
) -> Result<(), LoaderError> {
    let members: Vec<_> = judged.judged().cloned().collect();
    let fresh = super::fresh_extension_ids(migration_service, &members, prepared, target).await?;
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
    let tables = owners.recorded();
    let objects = recorded_object_owners(db).await?;
    let recorded = Recorded {
        tables: &tables,
        objects: &objects,
    };
    check_migration_ownership(judged, &recorded, &pending).map_err(|error| {
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
            let owner_judged = prepared.iter().any(|owner| {
                owner.extension_id == recorded.id && owner.standing == StandingTag::Install
            });
            let yields = (owner_judged && recorded.standing == StandingTag::Install)
                || handed_over(prepared, recorded, table);
            if yields && p.standing == StandingTag::Core {
                continue;
            }
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

fn handed_over(prepared: &[PreparedSchema], recorded: &RecordedOwner, table: &str) -> bool {
    recorded.standing == StandingTag::Core
        && prepared.iter().any(|owner| {
            owner.extension_id == recorded.id
                && owner.standing == StandingTag::Core
                && owner.retired_tables.contains(&qualify(table))
        })
}

pub(super) async fn record_owners(
    db: &dyn DatabaseProvider,
    prepared: &[PreparedSchema],
    members: MigrationMembers<'_>,
) -> Result<(), LoaderError> {
    let tables = recorded_table_owners(db).await?.recorded();
    let objects = recorded_object_owners(db).await?;
    let (owned, stale) = record_plan(
        members,
        &Recorded {
            tables: &tables,
            objects: &objects,
        },
    );
    release_stale_records(db, &stale).await?;
    record_object_owners(db, members, &owned).await?;
    if !recorded_table_owners_present(db).await? {
        return Ok(());
    }
    let declared =
        |p: &PreparedSchema| -> Vec<String> { p.owned_tables.iter().map(|t| qualify(t)).collect() };
    for p in prepared {
        db.execute(
            &RELEASE_SQL,
            &[&p.extension_id.as_str(), &declared(p), &p.retired_tables],
        )
        .await
        .map_err(|e| step_failed("Failed to release dropped table ownership", e))?;
    }
    for p in prepared {
        db.execute(
            &RECORD_SQL,
            &[&declared(p), &p.extension_id.as_str(), &p.standing.as_str()],
        )
        .await
        .map_err(|e| step_failed("Failed to record table ownership", e))?;
    }
    Ok(())
}

async fn recorded_table_owners_present(db: &dyn DatabaseProvider) -> Result<bool, LoaderError> {
    present(db, OWNERS_PRESENT_SQL, "table").await
}

pub(super) async fn present(
    db: &dyn DatabaseProvider,
    sql: &str,
    kind: &str,
) -> Result<bool, LoaderError> {
    let row = db.fetch_one(&sql, &[]).await.map_err(|e| {
        step_failed(
            &format!("Failed to look up the {kind} ownership records"),
            e,
        )
    })?;
    Ok(row
        .get("present")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false))
}

pub(super) fn step_failed(context: &str, source: crate::error::RepositoryError) -> LoaderError {
    LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("database"),
        context: context.to_owned(),
        source: Box::new(source),
    }
}
