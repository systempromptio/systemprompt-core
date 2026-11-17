//! Persisted object ownership: the member that owns each index, routine,
//! view, materialized view, sequence, type, domain and trigger.
//!
//! As `schema_table_owners` does for tables, `schema_object_owners` keeps a
//! disabled or held-back member's objects its own while its code is absent
//! or behind. After its DDL every install, full or settled:
//!
//! - deletes each stale record: one of an install-standing owner for a name a
//!   core member's SQL creates, and one of a judged member for an object its
//!   SQL no longer creates (a member's own record never counts as the create
//!   that keeps it); the same install-standing rule deletes a
//!   `schema_table_owners` record of a table a core member's SQL creates;
//! - for each judged member, deletes its records of objects the ownership rule
//!   no longer gives it and records those it does.
//!
//! A record of a held-back member, or of a member outside the composition, is
//! deleted only by the core rule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{LoaderError, StandingTag};
use systemprompt_identifiers::ExtensionId;

use super::super::migration_owners::{MigrationMembers, ObjectKind, RecordedObject, Stale};
use super::ownership::{present, step_failed};
use crate::services::DatabaseProvider;

const OBJECT_OWNERS_PRESENT_SQL: &str =
    "SELECT to_regclass('schema_object_owners') IS NOT NULL AS present";

const OBJECT_OWNERS_SQL: &str =
    "SELECT object_kind, object_name, extension_id, standing FROM schema_object_owners";

const RECORD_OBJECTS_SQL: &str = "INSERT INTO schema_object_owners \
     (object_kind, object_name, extension_id, standing) \
     SELECT d.kind, d.name, $3, $4 FROM unnest($1::text[], $2::text[]) AS d(kind, name) \
     ON CONFLICT (object_kind, object_name) DO NOTHING";

const RELEASE_STALE_OBJECTS_SQL: &str = "DELETE FROM schema_object_owners AS o \
     USING unnest($1::text[], $2::text[], $3::text[]) AS d(kind, name, owner) \
     WHERE o.object_kind = d.kind AND o.object_name = d.name AND o.extension_id = d.owner";

const RELEASE_STALE_TABLES_SQL: &str = "DELETE FROM schema_table_owners AS o \
     USING unnest($1::text[], $2::text[]) AS d(name, owner) \
     WHERE o.extension_id = d.owner \
     AND (o.table_name = d.name OR 'public.' || o.table_name = d.name)";

const RELEASE_OBJECTS_SQL: &str = "DELETE FROM schema_object_owners AS o \
     WHERE o.extension_id = $1 AND NOT EXISTS (\
     SELECT 1 FROM unnest($2::text[], $3::text[]) AS d(kind, name) \
     WHERE d.kind = o.object_kind AND d.name = o.object_name)";

pub(super) async fn recorded_object_owners(
    db: &dyn DatabaseProvider,
) -> Result<Vec<RecordedObject>, LoaderError> {
    if !present(db, OBJECT_OWNERS_PRESENT_SQL, "object").await? {
        return Ok(Vec::new());
    }
    let rows = db
        .fetch_all(&OBJECT_OWNERS_SQL, &[])
        .await
        .map_err(|e| step_failed("Failed to read the object ownership records", e))?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            Some(RecordedObject {
                kind: ObjectKind::parse(row.get("object_kind")?.as_str()?)?,
                name: row.get("object_name")?.as_str()?.to_owned(),
                owner: ExtensionId::new(row.get("extension_id")?.as_str()?),
                standing: StandingTag::parse(row.get("standing")?.as_str()?)?,
            })
        })
        .collect())
}

pub(super) async fn release_stale_records(
    db: &dyn DatabaseProvider,
    stale: &Stale,
) -> Result<(), LoaderError> {
    if !stale.objects.is_empty() {
        let (kinds, names, owners) = stale.objects.iter().fold(
            (Vec::new(), Vec::new(), Vec::new()),
            |(mut kinds, mut names, mut owners), record| {
                kinds.push(record.kind.as_str().to_owned());
                names.push(record.name.clone());
                owners.push(record.owner.as_str().to_owned());
                (kinds, names, owners)
            },
        );
        db.execute(&RELEASE_STALE_OBJECTS_SQL, &[&kinds, &names, &owners])
            .await
            .map_err(|e| step_failed("Failed to release stale object ownership", e))?;
    }
    if !stale.tables.is_empty() {
        let (names, owners): (Vec<String>, Vec<String>) = stale
            .tables
            .iter()
            .map(|record| (record.table.clone(), record.owner.as_str().to_owned()))
            .unzip();
        db.execute(&RELEASE_STALE_TABLES_SQL, &[&names, &owners])
            .await
            .map_err(|e| step_failed("Failed to release stale table ownership", e))?;
    }
    Ok(())
}

pub(super) async fn record_object_owners(
    db: &dyn DatabaseProvider,
    members: MigrationMembers<'_>,
    owned: &[RecordedObject],
) -> Result<(), LoaderError> {
    if !present(db, OBJECT_OWNERS_PRESENT_SQL, "object").await? {
        return Ok(());
    }
    for member in members.judged() {
        let id = ExtensionId::new(member.id());
        let (kinds, names): (Vec<String>, Vec<String>) = owned
            .iter()
            .filter(|record| record.owner == id)
            .map(|record| (record.kind.as_str().to_owned(), record.name.clone()))
            .unzip();
        db.execute(&RELEASE_OBJECTS_SQL, &[&id.as_str(), &kinds, &names])
            .await
            .map_err(|e| step_failed("Failed to release dropped object ownership", e))?;
        db.execute(
            &RECORD_OBJECTS_SQL,
            &[&kinds, &names, &id.as_str(), &member.standing().as_str()],
        )
        .await
        .map_err(|e| step_failed("Failed to record object ownership", e))?;
    }
    Ok(())
}
