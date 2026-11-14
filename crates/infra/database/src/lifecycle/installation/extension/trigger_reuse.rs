//! Per-trigger reuse inside a schema phase.
//!
//! A declarative trigger is written `DROP TRIGGER IF EXISTS … ON t` then
//! `CREATE TRIGGER … ON t` (or a lone `CREATE OR REPLACE TRIGGER`), and either
//! form takes an exclusive lock on `t`. When anything in the schema changes
//! the settled check fails and every phase re-runs, so without this a deploy
//! that touches one view still re-creates every trigger on every busy table.
//!
//! Each trigger's statements are digested, and after a phase applies them the
//! digest is recorded in `schema_trigger_digests` with the definition
//! Postgres then reports (`pg_get_triggerdef`). A later phase leaves a
//! trigger's statements out only when its declared digest is the recorded one
//! and the live definition still equals the recorded definition, so a trigger
//! an operator, an older binary or a restore dropped or changed is re-applied.
//! The check, the remaining statements and the new records run in the phase's
//! one transaction under the bootstrap lock. A trigger whose statements this
//! module cannot read as one such group is always applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};
use std::hash::Hasher;

use pg_query::NodeEnum;
use pg_query::protobuf::ObjectType;
use xxhash_rust::xxh64::Xxh64;

use crate::DatabaseResult;
use crate::models::DatabaseTransaction;

const DIGEST_TABLE_PRESENT_SQL: &str =
    "SELECT to_regclass('schema_trigger_digests') IS NOT NULL AS present";

const TRIGGER_UNCHANGED_SQL: &str = "SELECT 1 AS unchanged FROM schema_trigger_digests d \
     JOIN pg_trigger t ON t.tgrelid = to_regclass(d.relation) AND t.tgname = d.trigger_name \
     AND NOT t.tgisinternal \
     WHERE d.relation = $1 AND d.trigger_name = $2 AND d.declared = $3 \
     AND d.live_definition = pg_get_triggerdef(t.oid)";

const RECORD_TRIGGER_SQL: &str = "INSERT INTO schema_trigger_digests \
     (relation, trigger_name, declared, live_definition) \
     SELECT $1, $2, $3, pg_get_triggerdef(t.oid) FROM pg_trigger t \
     WHERE t.tgrelid = to_regclass($1) AND t.tgname = $2 AND NOT t.tgisinternal \
     ON CONFLICT (relation, trigger_name) DO UPDATE \
     SET declared = EXCLUDED.declared, live_definition = EXCLUDED.live_definition, \
     recorded_at = CURRENT_TIMESTAMP";

const FIELD_END: &[u8] = &[0x1f];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TriggerReuse {
    Reuse,
    Reapply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DeclaredTrigger {
    relation: String,
    name: String,
    declared: String,
    statements: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerStatementKind {
    Create,
    DropIfExists,
    Other,
}

pub(super) fn declared_triggers(statements: &[String]) -> Vec<DeclaredTrigger> {
    let mut groups: BTreeMap<(String, String), Vec<(usize, TriggerStatementKind)>> =
        BTreeMap::new();
    for (idx, statement) in statements.iter().enumerate() {
        if let Some((relation, name, kind)) = trigger_statement(statement) {
            groups
                .entry((relation, name))
                .or_default()
                .push((idx, kind));
        }
    }
    groups
        .into_iter()
        .filter(|(_, members)| reusable_group(members))
        .map(|((relation, name), members)| {
            let indices: Vec<usize> = members.iter().map(|(idx, _)| *idx).collect();
            let declared = digest(indices.iter().map(|idx| statements[*idx].as_str()));
            DeclaredTrigger {
                relation,
                name,
                declared,
                statements: indices,
            }
        })
        .collect()
}

fn reusable_group(members: &[(usize, TriggerStatementKind)]) -> bool {
    let creates = members
        .iter()
        .filter(|(_, kind)| *kind == TriggerStatementKind::Create)
        .count();
    let last_is_create = members
        .last()
        .is_some_and(|(_, kind)| *kind == TriggerStatementKind::Create);
    let rest_drop = members[..members.len().saturating_sub(1)]
        .iter()
        .all(|(_, kind)| *kind == TriggerStatementKind::DropIfExists);
    creates == 1 && last_is_create && rest_drop
}

fn digest<'a>(statements: impl Iterator<Item = &'a str>) -> String {
    let mut hasher = Xxh64::new(0);
    for statement in statements {
        hasher.write(statement.as_bytes());
        hasher.write(FIELD_END);
    }
    format!("{:016x}", hasher.finish())
}

fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn trigger_statement(statement: &str) -> Option<(String, String, TriggerStatementKind)> {
    let parsed = pg_query::parse(statement).ok()?;
    let [raw] = parsed.protobuf.stmts.as_slice() else {
        return None;
    };
    match raw.stmt.as_ref()?.node.as_ref()? {
        NodeEnum::CreateTrigStmt(create) => {
            let relation = create.relation.as_ref()?;
            let qualified = if relation.schemaname.is_empty() {
                quote(&relation.relname)
            } else {
                format!(
                    "{}.{}",
                    quote(&relation.schemaname),
                    quote(&relation.relname)
                )
            };
            let kind = if create.isconstraint {
                TriggerStatementKind::Other
            } else {
                TriggerStatementKind::Create
            };
            Some((qualified, create.trigname.clone(), kind))
        },
        NodeEnum::DropStmt(drop) if drop.remove_type == ObjectType::ObjectTrigger as i32 => {
            let [object] = drop.objects.as_slice() else {
                return None;
            };
            let Some(NodeEnum::List(list)) = object.node.as_ref() else {
                return None;
            };
            let mut parts: Vec<String> = list
                .items
                .iter()
                .map(|item| match item.node.as_ref() {
                    Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            let name = parts.pop()?;
            if parts.is_empty() {
                return None;
            }
            let relation = parts.iter().map(|p| quote(p)).collect::<Vec<_>>().join(".");
            let kind = if drop.missing_ok {
                TriggerStatementKind::DropIfExists
            } else {
                TriggerStatementKind::Other
            };
            Some((relation, name, kind))
        },
        _ => None,
    }
}

pub(super) async fn unchanged_statements(
    tx: &mut dyn DatabaseTransaction,
    triggers: &[DeclaredTrigger],
) -> DatabaseResult<HashSet<usize>> {
    let mut skip = HashSet::new();
    if triggers.is_empty() || !digest_table_present(tx).await? {
        return Ok(skip);
    }
    for trigger in triggers {
        let row = tx
            .fetch_optional(
                &TRIGGER_UNCHANGED_SQL,
                &[&trigger.relation, &trigger.name, &trigger.declared],
            )
            .await?;
        if row.is_some() {
            skip.extend(trigger.statements.iter().copied());
        }
    }
    Ok(skip)
}

pub(super) async fn record(
    tx: &mut dyn DatabaseTransaction,
    triggers: &[DeclaredTrigger],
) -> DatabaseResult<()> {
    if triggers.is_empty() || !digest_table_present(tx).await? {
        return Ok(());
    }
    for trigger in triggers {
        tx.execute(
            &RECORD_TRIGGER_SQL,
            &[&trigger.relation, &trigger.name, &trigger.declared],
        )
        .await?;
    }
    Ok(())
}

async fn digest_table_present(tx: &mut dyn DatabaseTransaction) -> DatabaseResult<bool> {
    let row = tx.fetch_one(&DIGEST_TABLE_PRESENT_SQL, &[]).await?;
    Ok(row
        .get("present")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false))
}
