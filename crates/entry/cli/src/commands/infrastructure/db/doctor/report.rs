//! The `db doctor` report and the notes rendered with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::SchemaResidue;
use systemprompt_extension::cli::CommandOutput;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::artifacts::NoticeLine;

use super::{DoctorReport, MissingColumn, UnparseableSchema};

pub(super) fn report_output(
    undeclared: &[String],
    residue: &SchemaResidue,
    missing_tables: &[String],
    missing_columns: &[(ExtensionId, String, String)],
    unparseable_schemas: Vec<UnparseableSchema>,
) -> CommandOutput {
    let orphan_ledgers: Vec<String> = residue
        .orphan_migration_ledgers
        .iter()
        .map(|l| format!("{} ({} rows)", l.extension_id, l.rows))
        .collect();
    let absent_tables: Vec<String> = residue
        .absent_member_tables
        .iter()
        .map(|t| {
            format!(
                "{}.{} ({} rows, owner {})",
                t.schema, t.table, t.live_rows, t.owner
            )
        })
        .collect();
    let absent_ledgers: Vec<String> = residue
        .absent_member_ledgers
        .iter()
        .map(|l| format!("{} ({} rows)", l.extension_id, l.rows))
        .collect();
    let report = DoctorReport {
        undeclared_tables: undeclared.to_vec(),
        orphan_migration_ledgers: orphan_ledgers,
        absent_member_tables: absent_tables,
        absent_member_ledgers: absent_ledgers,
        missing_tables: missing_tables.to_vec(),
        missing_columns: missing_columns
            .iter()
            .map(|(ext, table, col)| MissingColumn {
                extension: ext.clone(),
                table: table.clone(),
                column: col.clone(),
            })
            .collect(),
        unparseable_schemas,
    };
    report_notes(&report).into_iter().fold(
        CommandOutput::card_value("Database Doctor", &report),
        CommandOutput::with_note,
    )
}

fn report_notes(report: &DoctorReport) -> Vec<NoticeLine> {
    let mut notes = Vec::new();
    if !report.absent_member_tables.is_empty() || !report.absent_member_ledgers.is_empty() {
        notes.push(NoticeLine::new(
            "info",
            format!(
                "{} table(s) and {} migration ledger(s) are kept for a member not in this \
                 composition; they are not residue and must not be dropped",
                report.absent_member_tables.len(),
                report.absent_member_ledgers.len()
            ),
        ));
    }
    if report.undeclared_tables.is_empty()
        && report.unparseable_schemas.is_empty()
        && report.orphan_migration_ledgers.is_empty()
        && report.missing_tables.is_empty()
        && report.missing_columns.is_empty()
    {
        notes.push(NoticeLine::new(
            "success",
            "Schema in sync with extension declarations",
        ));
    }
    if !report.missing_tables.is_empty() {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "{} declared table(s) absent from the live database",
                report.missing_tables.len()
            ),
        ));
    }
    if !report.missing_columns.is_empty() {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "{} required column(s) absent from live tables",
                report.missing_columns.len()
            ),
        ));
    }
    if !report.undeclared_tables.is_empty() {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "{} live table(s) declared by no registered extension — a deleted crate's \
                 residue; add a DROP TABLE … CASCADE migration",
                report.undeclared_tables.len()
            ),
        ));
    }
    if !report.orphan_migration_ledgers.is_empty() {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "{} extension_migrations ledger(s) for extensions that no longer exist — delete \
                 the rows in the same migration",
                report.orphan_migration_ledgers.len()
            ),
        ));
    }
    notes
}
