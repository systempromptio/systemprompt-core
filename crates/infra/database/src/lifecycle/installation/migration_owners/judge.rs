//! The judgement of each pending migration statement against the ownership
//! of the tables and objects it names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_extension::{Migration, StandingTag};
use systemprompt_identifiers::ExtensionId;
use tracing::warn;

use super::super::undeclared::qualify;
use super::claims::{Ownership, ownership};
use super::kinds::HiddenSql;
use super::statements::{Effect, effects_of, top_level};
use super::{
    KERNEL_LEDGER_OWNER, KERNEL_LEDGERS, MigrationMembers, MigrationOwnershipError,
    RecordedTable, TableMutation, label,
};

struct Judged<'a> {
    id: &'a ExtensionId,
    standing: StandingTag,
    migration: &'a str,
    cross: &'a BTreeSet<String>,
}

impl Judged<'_> {
    fn install(&self) -> bool {
        self.standing == StandingTag::Install
    }

    fn unjudged(&self, statement: &dyn std::fmt::Display, object: &str) {
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
    recorded: &[RecordedTable],
    pending: &dyn Fn(&ExtensionId, &Migration) -> bool,
) -> Vec<MigrationOwnershipError> {
    let mut findings = Vec::new();
    let ownership = ownership(members, recorded, &mut findings);
    for member in members.judged {
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
                &format!("migration {migration_label}"),
                migration.sql,
            ));
            let judged = Judged {
                id: &id,
                standing: member.standing(),
                migration: &migration_label,
                cross: &cross,
            };
            for effect in top_level(&parsed).flat_map(effects_of) {
                findings.extend(judge(&ownership, &judged, effect));
            }
        }
    }
    findings
}

const fn logged_table_statement(statement: TableMutation) -> bool {
    matches!(
        statement,
        TableMutation::CreateIndex | TableMutation::CreateTrigger | TableMutation::DropTrigger
    )
}

const fn logged_hidden(statement: &HiddenSql) -> bool {
    matches!(statement, HiddenSql::Routine { .. } | HiddenSql::RoutineCall)
}

fn judge(
    ownership: &Ownership,
    member: &Judged<'_>,
    effect: Effect,
) -> Option<MigrationOwnershipError> {
    let migration = member.migration.to_owned();
    let id = member.id.clone();
    match effect {
        Effect::Table(statement, table) => {
            let owner = ownership.owner(&table).filter(|owner| owner.id != *member.id)?;
            let admitted_cross = member.cross.contains(&table)
                && !(member.install() && owner.standing == StandingTag::Core);
            if admitted_cross {
                return None;
            }
            if logged_table_statement(statement) {
                member.unjudged(&statement, &table);
                return None;
            }
            Some(MigrationOwnershipError::ForeignTable {
                member: id,
                migration,
                statement,
                table,
                owner: owner.id.clone(),
            })
        },
        Effect::Write(statement, table, pinned) => {
            if !KERNEL_LEDGERS.contains(&table.as_str()) || id.as_str() == KERNEL_LEDGER_OWNER {
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
        Effect::Object(statement, object) => judge_object(ownership, member, statement, object),
        Effect::Cascade(object) => {
            let own = ownership.created.get(member.id).is_some_and(|own| {
                own.objects.contains(&object) || own.indexes.contains(&object)
            });
            if !own {
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

fn judge_schema(
    ownership: &Ownership,
    member: &Judged<'_>,
    statement: TableMutation,
    schema: Option<String>,
) -> Option<MigrationOwnershipError> {
    let refused = schema.as_deref().is_none_or(|schema| {
        schema == "public" || ownership.held_by_another(schema, member.id)
    });
    if !refused {
        member.unjudged(&statement, schema.as_deref().unwrap_or_default());
        return None;
    }
    Some(MigrationOwnershipError::SchemaLevel {
        member: member.id.clone(),
        migration: member.migration.to_owned(),
        statement,
        schema: schema.unwrap_or_else(|| String::from("(every schema)")),
    })
}

fn judge_object(
    ownership: &Ownership,
    member: &Judged<'_>,
    statement: TableMutation,
    object: String,
) -> Option<MigrationOwnershipError> {
    let index = matches!(statement, TableMutation::DropIndex | TableMutation::AlterIndex);
    let creators: Vec<ExtensionId> = if index {
        ownership.creators(|c| &c.indexes, &object).cloned().collect()
    } else {
        ownership.creators(|c| &c.objects, &object).cloned().collect()
    };
    if creators.contains(member.id) {
        return None;
    }
    match creators.first() {
        Some(owner) if member.install() => Some(MigrationOwnershipError::ForeignObject {
            member: member.id.clone(),
            migration: member.migration.to_owned(),
            statement,
            object,
            owner: owner.clone(),
        }),
        Some(_) => {
            member.unjudged(&statement, &object);
            None
        },
        None => {
            if index {
                member.unjudged(&statement, &object);
            }
            None
        },
    }
}
