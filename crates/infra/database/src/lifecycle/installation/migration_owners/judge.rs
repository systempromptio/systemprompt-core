//! The judgement of each pending migration statement, and of each statement
//! of an install member's declarative schema (which every install runs),
//! against the ownership of the tables and objects it names. Both go through
//! one path: the statement's creates of another member's object, then every
//! effect the reader extracts from it, each judged by the same rules. A core
//! member's declarative schema is not judged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_extension::{Migration, StandingTag};
use systemprompt_identifiers::ExtensionId;
use tracing::warn;

use super::super::undeclared::qualify;
use super::claims::{Claim, Ownership, ownership};
use super::judge_objects::{foreign_object_creates, judge_named, judge_schema};
use super::kinds::HiddenSql;
use super::ledger::ledger_reads;
use super::object_claims::Prefer;
use super::statements::{Effect, effects_of, top_level};
use super::{
    KERNEL_LEDGER_OWNER, KERNEL_LEDGERS, MigrationMembers, MigrationOwnershipError, Recorded,
    TableMutation, label,
};

pub(super) struct Judged<'a> {
    pub(super) id: &'a ExtensionId,
    pub(super) standing: StandingTag,
    pub(super) migration: &'a str,
    cross: &'a BTreeSet<String>,
}

impl Judged<'_> {
    pub(super) fn install(&self) -> bool {
        self.standing == StandingTag::Install
    }

    pub(super) fn owns(&self, claim: &Claim) -> bool {
        claim.held_by(self.id, self.standing)
    }

    pub(super) fn unjudged(&self, statement: &dyn std::fmt::Display, object: &str) {
        warn!(
            member = %self.id,
            migration = %self.migration,
            statement = %statement,
            object = %object,
            "A migration statement is not judged for what it changes"
        );
    }
}

pub(super) fn findings(
    members: MigrationMembers<'_>,
    recorded: &Recorded<'_>,
    pending: &dyn Fn(&ExtensionId, &Migration) -> bool,
) -> Vec<MigrationOwnershipError> {
    let mut findings = Vec::new();
    let ownership = ownership(members, recorded, &mut findings);
    for member in members.judged() {
        let id = ExtensionId::new(member.id());
        let cross: BTreeSet<String> = member
            .cross_extension_tables()
            .into_iter()
            .map(qualify)
            .collect();
        for migration in member.migrations().into_iter().filter(|m| !m.tombstone) {
            if !pending(&id, &migration) {
                continue;
            }
            let migration_label = label(&migration);
            let parsed = match pg_query::parse(migration.sql) {
                Ok(parsed) => parsed,
                Err(source) => {
                    findings.push(MigrationOwnershipError::Parse {
                        member: id.clone(),
                        migration: migration_label,
                        source,
                    });
                    continue;
                },
            };
            findings.extend(ownership.foreign_creates(
                &id,
                member.standing(),
                &format!("migration {migration_label}"),
                migration.sql,
            ));
            let judged = Judged {
                id: &id,
                standing: member.standing(),
                migration: &migration_label,
                cross: &cross,
            };
            judge_statements(&ownership, &judged, &parsed, &mut findings);
        }
        if member.standing() != StandingTag::Install {
            continue;
        }
        let judged = Judged {
            id: &id,
            standing: member.standing(),
            migration: "schema",
            cross: &cross,
        };
        for schema in member.schemas() {
            if let Ok(parsed) = pg_query::parse(&schema.sql) {
                judge_statements(&ownership, &judged, &parsed, &mut findings);
            }
        }
    }
    findings
}

fn judge_statements(
    ownership: &Ownership,
    judged: &Judged<'_>,
    parsed: &pg_query::ParseResult,
    findings: &mut Vec<MigrationOwnershipError>,
) {
    for statement in top_level(parsed) {
        findings.extend(foreign_object_creates(ownership, judged, statement));
        if judged.install() {
            findings.extend(
                ledger_reads(statement)
                    .into_iter()
                    .map(|(statement, table)| MigrationOwnershipError::ForeignTable {
                        member: judged.id.clone(),
                        migration: judged.migration.to_owned(),
                        statement,
                        table,
                        owner: ExtensionId::new(KERNEL_LEDGER_OWNER),
                    }),
            );
        }
    }
    let mut hidden = Vec::new();
    for effect in top_level(parsed).flat_map(effects_of) {
        if let Effect::Hidden(statement) = &effect {
            if hidden.contains(statement) {
                continue;
            }
            hidden.push(statement.clone());
        }
        findings.extend(judge(ownership, judged, effect));
    }
}

fn kernel(member: &Judged<'_>) -> bool {
    member.id.as_str() == KERNEL_LEDGER_OWNER && member.standing == StandingTag::Core
}

const fn logged_table_statement(statement: TableMutation) -> bool {
    matches!(
        statement,
        TableMutation::CreateIndex | TableMutation::CreateTrigger | TableMutation::DropTrigger
    )
}

const fn logged_hidden(statement: &HiddenSql) -> bool {
    matches!(
        statement,
        HiddenSql::Do | HiddenSql::Routine { .. } | HiddenSql::RoutineCall | HiddenSql::Extension
    )
}

fn judge(
    ownership: &Ownership,
    member: &Judged<'_>,
    effect: Effect,
) -> Option<MigrationOwnershipError> {
    let migration = member.migration.to_owned();
    let id = member.id.clone();
    match effect {
        Effect::Table(statement, table) => judge_table(ownership, member, statement, table),
        Effect::Write(statement, table, pinned) => {
            if !KERNEL_LEDGERS.contains(&table.as_str()) || kernel(member) {
                return None;
            }
            let removed_members_only = statement == TableMutation::Delete
                && table == "public.extension_migrations"
                && pinned.is_some_and(|ids| {
                    !ids.is_empty() && ids.iter().all(|pinned| !ownership.knows(pinned))
                });
            (!removed_members_only).then_some(MigrationOwnershipError::LedgerWrite {
                member: id,
                migration,
                statement,
                table,
            })
        },
        Effect::Schema(statement, schema) => judge_schema(ownership, member, statement, schema),
        Effect::Object(statement, object) => {
            let prefer = if matches!(
                statement,
                TableMutation::DropIndex | TableMutation::AlterIndex
            ) {
                Prefer::Index
            } else {
                Prefer::Object
            };
            judge_named(ownership, member, statement, object, prefer)
        },
        Effect::Cascade(object) => {
            if member.install() || !ownership.owns_anything_named(member.id, &object) {
                member.unjudged(&TableMutation::DropCascade, &object);
            }
            None
        },
        Effect::Hidden(statement) => {
            if member.install() && !logged_hidden(&statement) {
                return Some(MigrationOwnershipError::Unreadable {
                    member: id,
                    migration,
                    statement,
                });
            }
            member.unjudged(&statement, "");
            None
        },
    }
}

fn judge_table(
    ownership: &Ownership,
    member: &Judged<'_>,
    statement: TableMutation,
    table: String,
) -> Option<MigrationOwnershipError> {
    let Some(owner) = ownership.owner(&table) else {
        if logged_table_statement(statement) {
            if ownership
                .resolve(&table, Prefer::Table)
                .is_some_and(|owner| !member.owns(owner))
            {
                member.unjudged(&statement, &table);
            }
            return None;
        }
        return judge_named(ownership, member, statement, table, Prefer::Table);
    };
    if member.owns(owner) {
        return None;
    }
    let ledger = KERNEL_LEDGERS.contains(&table.as_str());
    let admitted_cross = !ledger
        && member.cross.contains(&table)
        && !(member.install() && owner.standing == StandingTag::Core);
    if admitted_cross {
        return None;
    }
    if logged_table_statement(statement) && !ledger {
        member.unjudged(&statement, &table);
        return None;
    }
    Some(MigrationOwnershipError::ForeignTable {
        member: member.id.clone(),
        migration: member.migration.to_owned(),
        statement,
        table,
        owner: owner.id.clone(),
    })
}
