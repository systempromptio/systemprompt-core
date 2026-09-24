//! A migration never runs the row triggers on the tables it writes, unless it
//! declares `@cost … triggers=live`.
//!
//! Migrations run before the dependent phase that recreates declarative
//! triggers, so every trigger live during a migration belongs to the
//! *previous* release — and often to another extension. On a 0.58 → 0.60
//! upgrade core ai 032's `UPDATE ai_requests` ran astound's
//! `feedback_capture` once per row and hit its `statement_timeout`; web 093
//! then fired the same trigger after core analytics 015 had dropped the table
//! its function writes, and failed outright. The extension that retires those
//! triggers migrates after core, so no ordering of migrations fixes this: the
//! runner has to suspend them.
//!
//! Suspension is `ALTER TABLE … DISABLE TRIGGER`, which needs table
//! ownership, not `session_replication_role`, which needs superuser and is
//! not granted on managed databases. Only triggers enabled when the migration
//! starts are disabled and exactly those are re-enabled, so a trigger an
//! operator (or the migration itself) disabled stays disabled, and one the
//! migration dropped is skipped. Internal constraint triggers are never
//! touched, so foreign keys are still checked.
//!
//! On the transactional path both halves run in the migration's transaction.
//! On the `@no-transaction` path they are separate statements, and the
//! restore runs on success and on failure alike; a process killed between the
//! two leaves the triggers disabled until the dependent phase recreates them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use crate::DatabaseTransaction;
use crate::error::DatabaseResult;
use crate::models::{JsonRow, ToDbValue};
use crate::services::DatabaseProvider;
use pg_query::Context;
use systemprompt_extension::{Migration, TriggerPolicy, cost};

const ENABLED_USER_TRIGGERS: &str = "SELECT t.tgname::text AS name FROM pg_trigger t WHERE \
                                     t.tgrelid = to_regclass($1) AND NOT t.tgisinternal AND \
                                     t.tgenabled <> 'D' ORDER BY t.tgname";

const TRIGGER_PRESENT: &str =
    "SELECT 1 AS present FROM pg_trigger WHERE tgrelid = to_regclass($1) AND tgname = $2";

pub(super) enum Target<'a> {
    Tx(&'a mut dyn DatabaseTransaction),
    Pool(&'a dyn DatabaseProvider),
}

impl Target<'_> {
    async fn execute(&mut self, sql: &str) -> DatabaseResult<u64> {
        match self {
            Self::Tx(tx) => tx.execute(&sql, &[]).await,
            Self::Pool(db) => db.execute(&sql, &[]).await,
        }
    }

    async fn fetch_all(
        &mut self,
        sql: &str,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>> {
        match self {
            Self::Tx(tx) => tx.fetch_all(&sql, params).await,
            Self::Pool(db) => db.fetch_all(&sql, params).await,
        }
    }

    async fn fetch_optional(
        &mut self,
        sql: &str,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>> {
        match self {
            Self::Tx(tx) => tx.fetch_optional(&sql, params).await,
            Self::Pool(db) => db.fetch_optional(&sql, params).await,
        }
    }
}

#[derive(Default)]
pub(super) struct Suspended {
    triggers: Vec<(String, String)>,
}

// Why: a malformed `@cost` is refused at build time, so an unparseable one
// here cannot reach a customer; treating it as "suspend" keeps the safe side.
#[must_use]
pub(super) fn triggers_live(migration: &Migration) -> bool {
    matches!(
        cost::parse(migration.sql),
        Ok(Some(directive)) if directive.triggers == TriggerPolicy::Live
    )
}

fn written_tables(sql: &str) -> Result<BTreeSet<String>, String> {
    let parsed = pg_query::parse(sql).map_err(|e| e.to_string())?;
    Ok(parsed
        .tables
        .iter()
        .filter(|(_, context)| matches!(context, Context::DML))
        .map(|(table, _)| table.clone())
        .collect())
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn quote_table(table: &str) -> String {
    table
        .split('.')
        .map(quote_ident)
        .collect::<Vec<_>>()
        .join(".")
}

pub(super) async fn suspend(
    target: &mut Target<'_>,
    migration: &Migration,
) -> Result<Suspended, String> {
    if triggers_live(migration) {
        return Ok(Suspended::default());
    }
    let tables = written_tables(migration.sql).map_err(|e| {
        format!(
            "Failed to parse migration {} ({}) for trigger suspension: {e}",
            migration.version, migration.name
        )
    })?;
    let mut suspended = Suspended::default();
    for table in tables {
        let rows = target
            .fetch_all(ENABLED_USER_TRIGGERS, &[&table])
            .await
            .map_err(|e| format!("Failed to list triggers on {table}: {e}"))?;
        for row in rows {
            let Some(name) = row.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            let sql = format!(
                "ALTER TABLE {} DISABLE TRIGGER {}",
                quote_table(&table),
                quote_ident(name)
            );
            target
                .execute(&sql)
                .await
                .map_err(|e| format!("Failed to suspend trigger {name} on {table}: {e}"))?;
            suspended.triggers.push((table.clone(), name.to_owned()));
        }
    }
    Ok(suspended)
}

impl Suspended {
    pub(super) const fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }

    pub(super) fn describe(&self) -> String {
        self.triggers
            .iter()
            .map(|(table, name)| format!("{table}.{name}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(super) async fn restore(self, target: &mut Target<'_>) -> Result<(), String> {
        for (table, name) in &self.triggers {
            // Why: a migration may drop a trigger it just had suspended;
            // restoring one that is gone is not a failure.
            let present = target
                .fetch_optional(TRIGGER_PRESENT, &[table, name])
                .await
                .map_err(|e| format!("Failed to check trigger {name} on {table}: {e}"))?;
            if present.is_none() {
                continue;
            }
            let sql = format!(
                "ALTER TABLE {} ENABLE TRIGGER {}",
                quote_table(table),
                quote_ident(name)
            );
            target
                .execute(&sql)
                .await
                .map_err(|e| format!("Failed to restore trigger {name} on {table}: {e}"))?;
        }
        Ok(())
    }
}
