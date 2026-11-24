//! Owner reassignments and user purges judged against the catalog.
//!
//! Kernel admission accepts a member's owner reassignment over a table the
//! member's own schema text creates. The text is not the database: a
//! `CREATE TABLE IF NOT EXISTS` names a table whether or not the statement
//! created it, and the live table may lack the column the declaration names.
//! The install therefore judges every declaration again once its DDL has run,
//! against `pg_attribute` and the ownership records:
//!
//! - before any DDL, a reassignment over a table in
//!   [`MERGE_EXCLUDED_SECURITY_TABLES`] is refused whatever the member
//!   declares ([`refuse_security_tables`]);
//! - after the ownership records are written, a reassignment target must be a
//!   live table, carry every column the declaration names, and not be recorded
//!   to another member; a purge target present in the catalog must not be
//!   recorded to another member ([`check_catalog_targets`]).
//!
//! Each refusal is [`LoaderError::ReassignmentTargetRefused`] with a typed
//! [`ReassignmentRefusal`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_extension::{LoaderError, PurgeDeclaration, ReassignmentRefusal};
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::MERGE_EXCLUDED_SECURITY_TABLES;

use super::ExtensionSet;
use super::ownership::{TableOwners, recorded_table_owners, step_failed};
use crate::services::DatabaseProvider;

const LIVE_COLUMNS_SQL: &str = "SELECT a.attname::text AS name FROM pg_attribute a \
     WHERE a.attrelid = to_regclass($1) AND a.attnum > 0 AND NOT a.attisdropped";

pub(super) fn refuse_security_tables(members: &ExtensionSet) -> Result<(), LoaderError> {
    for member in members {
        if let Some(reassignment) = member
            .reassignments()
            .into_iter()
            .find(|r| MERGE_EXCLUDED_SECURITY_TABLES.contains(&r.table))
        {
            return Err(refused(
                member.id(),
                reassignment.table,
                ReassignmentRefusal::SecurityTable,
            ));
        }
    }
    Ok(())
}

pub(super) async fn check_catalog_targets(
    db: &dyn DatabaseProvider,
    members: &ExtensionSet,
) -> Result<(), LoaderError> {
    let owners = recorded_table_owners(db).await?;
    for member in members {
        for reassignment in member.reassignments() {
            refuse_peer_record(&owners, member.id(), reassignment.table)?;
            let live = live_columns(db, reassignment.table).await?;
            if live.is_empty() {
                return Err(refused(
                    member.id(),
                    reassignment.table,
                    ReassignmentRefusal::AbsentTable,
                ));
            }
            if let Some(column) = reassignment
                .identifiers()
                .into_iter()
                .skip(1)
                .find(|column| !live.contains(*column))
            {
                return Err(refused(
                    member.id(),
                    reassignment.table,
                    ReassignmentRefusal::AbsentColumn(column.to_owned()),
                ));
            }
        }
        for purge in member.purges() {
            if let PurgeDeclaration::Table { table, .. } = &purge {
                refuse_peer_record(&owners, member.id(), table)?;
            }
        }
    }
    Ok(())
}

fn refuse_peer_record(owners: &TableOwners, member: &str, table: &str) -> Result<(), LoaderError> {
    match owners.owner_of(table) {
        Some(owner) if owner.as_str() != member => Err(refused(
            member,
            table,
            ReassignmentRefusal::RecordedToPeer(owner.clone()),
        )),
        _ => Ok(()),
    }
}

async fn live_columns(
    db: &dyn DatabaseProvider,
    table: &str,
) -> Result<BTreeSet<String>, LoaderError> {
    let rows = db
        .fetch_all(&LIVE_COLUMNS_SQL, &[&table])
        .await
        .map_err(|e| step_failed("Failed to read a reassignment target's columns", e))?;
    Ok(rows
        .iter()
        .filter_map(|row| row.get("name").and_then(|v| v.as_str()).map(str::to_owned))
        .collect())
}

fn refused(member: &str, table: &str, refusal: ReassignmentRefusal) -> LoaderError {
    LoaderError::ReassignmentTargetRefused {
        member: ExtensionId::new(member),
        table: table.to_owned(),
        refusal,
    }
}
