//! The verdict `infra db migrate` exits on: a held-back member or a
//! `@compat: breaking` migration fails the command; an unmarked ahead
//! migration and a superseded install are warnings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_database::{HeldBackMember, InstallRefusal, SchemaInstallReport};
use systemprompt_extension::BuiltState;
use systemprompt_models::artifacts::NoticeLine;

use super::admin_migrate::install_set;

pub(super) fn refuse_incompatible(built: &BuiltState, report: &SchemaInstallReport) -> Result<()> {
    match report.refusal() {
        None => Ok(()),
        Some(InstallRefusal::HeldBack(held_back)) => refuse_held_back(built, held_back),
        Some(InstallRefusal::BreaksOlder(ahead)) => {
            let breaking: Vec<String> = ahead
                .iter()
                .filter(|m| m.breaks_older)
                .map(|m| format!("{} {} {}", m.extension, m.version, m.name))
                .collect();
            Err(anyhow!(
                "The database records {} migration(s) a newer release marked @compat: breaking; \
                 this release cannot read that schema:\n{}\nHint: run the newer release",
                breaking.len(),
                breaking.join("\n")
            ))
        },
    }
}

pub(super) fn ahead_warnings(report: &SchemaInstallReport) -> Vec<NoticeLine> {
    let mut notes: Vec<NoticeLine> = report
        .unmarked_ahead()
        .into_iter()
        .map(|m| {
            NoticeLine::new(
                "warning",
                format!(
                    "{}: the database records migration {} {} from a newer release; its schema \
                     stays readable by this one",
                    m.extension, m.version, m.name
                ),
            )
        })
        .collect();
    if let Some(superseded) = &report.superseded {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "Release {} installed this database; release {} applied no schema",
                superseded.recorded, superseded.release
            ),
        ));
    }
    notes
}

fn refuse_held_back(built: &BuiltState, held_back: &[HeldBackMember]) -> Result<()> {
    let installed: Vec<String> = install_set(built)
        .iter()
        .map(|ext| ext.id().to_owned())
        .filter(|id| !held_back.iter().any(|held| held.extension.as_str() == id))
        .collect();
    let held: Vec<String> = held_back.iter().map(ToString::to_string).collect();
    Err(anyhow!(
        "Schema installation held back {} extension(s); the database is not at this binary's \
         schema for them and none of their schema was applied:\n{}\nInstalled: {}\nHint: run \
         the release that wrote those migrations, or add a tombstone for a migration that \
         was never released",
        held.len(),
        held.join("\n"),
        installed.join(", ")
    ))
}
