//! Persisted object ownership: the member that owns each index, routine,
//! view, materialized view, sequence, type, domain and trigger.
//!
//! As `schema_table_owners` does for tables, `schema_object_owners` keeps a
//! disabled member's objects its own while its code is absent: every install
//! records, for each admitted member, the objects the ownership rule gives it
//! and releases the records of those it no longer creates; a record of a
//! member outside the composition is never touched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{LoaderError, StandingTag};
use systemprompt_identifiers::ExtensionId;

use super::super::migration_owners::{
    MigrationMembers, ObjectKind, Recorded, RecordedObject, owned_objects,
};
use super::ownership::{present, recorded_table_owners, step_failed};
use crate::services::DatabaseProvider;

const OBJECT_OWNERS_PRESENT_SQL: &str =
    "SELECT to_regclass('schema_object_owners') IS NOT NULL AS present";

const OBJECT_OWNERS_SQL: &str =
    "SELECT object_kind, object_name, extension_id, standing FROM schema_object_owners";

const RECORD_OBJECTS_SQL: &str = "INSERT INTO schema_object_owners \
     (object_kind, object_name, extension_id, standing) \
     SELECT d.kind, d.name, $3, $4 FROM unnest($1::text[], $2::text[]) AS d(kind, name) \
     ON CONFLICT (object_kind, object_name) DO NOTHING";

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

pub(super) async fn record_object_owners(
    db: &dyn DatabaseProvider,
    members: MigrationMembers<'_>,
) -> Result<(), LoaderError> {
    if !present(db, OBJECT_OWNERS_PRESENT_SQL, "object").await? {
        return Ok(());
    }
    let tables = recorded_table_owners(db).await?.recorded();
    let objects = recorded_object_owners(db).await?;
    let owned = owned_objects(
        members,
        &Recorded {
            tables: &tables,
            objects: &objects,
        },
    );
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
