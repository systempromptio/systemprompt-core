//! Refuses a boot that leaves an enabled trigger running a PL/pgSQL function
//! that reads or writes a relation that no longer exists.
//!
//! Postgres does not track what a PL/pgSQL body names, so `DROP TABLE`
//! succeeds while a trigger's function still writes that table, and the
//! failure surfaces on the next ordinary write — after boot, in a request,
//! far from the migration that caused it. On a 0.58 → 0.60 upgrade core
//! analytics 015 dropped `analytics_ingestion_producers` while astound's
//! `feedback_capture` triggers still wrote it. This check runs once the schema
//! is final, and names the trigger, its table, the function and the missing
//! relation, so the fix — retire the trigger in its extension's retirements —
//! is in the error rather than in a stack trace.
//!
//! Statements are extracted with `pg_query::parse_plpgsql` from
//! `pg_get_functiondef`. Dynamic SQL (`EXECUTE`) is opaque and not checked;
//! relations the function itself creates, and a statement trigger's
//! transition tables, are not references.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use pg_query::Context;
use systemprompt_extension::LoaderError;
use tracing::warn;

use crate::services::DatabaseProvider;

// Why: one function can serve several triggers (an INSERT one naming
// `new_rows`, a DELETE one naming `old_rows`) and branch on TG_OP, so every
// transition-table name any of its triggers declares is excluded.
const LIVE_PLPGSQL_TRIGGERS: &str = "SELECT t.tgname::text AS trigger, t.tgrelid::regclass::text \
                                     AS table_name, p.oid::regprocedure::text AS function, \
                                     pg_get_functiondef(p.oid) AS definition, (SELECT \
                                     COALESCE(string_agg(n, ','), '') FROM (SELECT s.tgnewtable::text \
                                     AS n FROM pg_trigger s WHERE s.tgfoid = p.oid UNION SELECT \
                                     s.tgoldtable::text FROM pg_trigger s WHERE s.tgfoid = p.oid) \
                                     names WHERE n IS NOT NULL) AS transition_tables FROM \
                                     pg_trigger t JOIN pg_proc p ON p.oid = t.tgfoid JOIN \
                                     pg_language l ON l.oid = p.prolang WHERE NOT \
                                     t.tgisinternal AND t.tgenabled <> 'D' AND l.lanname = \
                                     'plpgsql' ORDER BY 2, 1";

const RELATION_EXISTS: &str = "SELECT to_regclass($1) IS NOT NULL AS present";

fn collect_queries(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(expr) = map.get("PLpgSQL_expr")
                && expr.get("parseMode").and_then(serde_json::Value::as_i64) == Some(0)
                && let Some(query) = expr.get("query").and_then(|q| q.as_str())
            {
                out.push(query.to_owned());
            }
            for child in map.values() {
                collect_queries(child, out);
            }
        },
        serde_json::Value::Array(items) => {
            for child in items {
                collect_queries(child, out);
            }
        },
        _ => {},
    }
}

fn referenced_relations(definition: &str) -> Option<BTreeSet<String>> {
    let parsed = pg_query::parse_plpgsql(definition).ok()?;
    let mut queries = Vec::new();
    collect_queries(&parsed, &mut queries);
    let mut used = BTreeSet::new();
    let mut local = BTreeSet::new();
    for query in queries {
        let Ok(result) = pg_query::parse(&query) else {
            continue;
        };
        for (table, context) in &result.tables {
            match context {
                Context::DDL => {
                    local.insert(table.to_lowercase());
                },
                Context::Select | Context::DML => {
                    used.insert(table.to_lowercase());
                },
                _ => {},
            }
        }
    }
    Some(used.difference(&local).cloned().collect())
}

fn text(row: &crate::models::JsonRow, key: &str) -> String {
    row.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}

pub async fn check_trigger_routines(db: &dyn DatabaseProvider) -> Result<(), LoaderError> {
    let failed = |message: String| LoaderError::SchemaInstallationFailed {
        extension: "database".to_owned(),
        message,
    };
    let rows = db
        .fetch_all(&LIVE_PLPGSQL_TRIGGERS, &[])
        .await
        .map_err(|e| failed(format!("Failed to list trigger routines: {e}")))?;

    let mut first: Option<LoaderError> = None;
    for row in &rows {
        let definition = text(row, "definition");
        let function = text(row, "function");
        let Some(mut relations) = referenced_relations(&definition) else {
            warn!(function = %function, "Trigger routine body could not be parsed; not checked");
            continue;
        };
        for name in text(row, "transition_tables").split(',') {
            relations.remove(&name.to_lowercase());
        }
        for relation in relations {
            let present = db
                .fetch_optional(&RELATION_EXISTS, &[&relation])
                .await
                .map_err(|e| failed(format!("Failed to resolve {relation}: {e}")))?
                .and_then(|r| r.get("present").and_then(serde_json::Value::as_bool))
                .unwrap_or(false);
            if present {
                continue;
            }
            let err = LoaderError::DanglingTriggerRoutine {
                trigger: text(row, "trigger"),
                table: text(row, "table_name"),
                function: function.clone(),
                relation,
            };
            if first.is_some() {
                warn!(error = %err, "Further dangling trigger routine");
            } else {
                first = Some(err);
            }
        }
    }
    first.map_or(Ok(()), Err)
}
