//! The pending-migration cost warning: which pending migrations rewrite a
//! populated hot table without a measured `@cost` directive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::Extension;
use tracing::warn;

use crate::lifecycle::installation::migration_cost::{HOT_TABLES, audit_one};
use crate::lifecycle::migrations::MigrationService;
use crate::services::DatabaseProvider;

// Why: a warning, never a refusal, and scoped to migrations that are still
// pending against a table that already holds rows. Refusing here would turn a
// missing comment into a customer's instance that will not boot; the
// whole-catalogue check belongs to the test suites. See the module docs of
// `migration_cost`.
pub(super) async fn warn_unmeasured_migrations(
    db: &dyn DatabaseProvider,
    migration_service: &MigrationService<'_>,
    extensions: &[std::sync::Arc<dyn Extension>],
) {
    let mut pending = Vec::new();
    for ext in extensions {
        let extension = ext.id().to_owned();
        let applied: std::collections::HashSet<u32> = migration_service
            .get_applied_migrations(&extension)
            .await
            .map(|rows| rows.into_iter().map(|row| row.version).collect())
            .unwrap_or_default();
        for migration in ext.migrations().into_iter().filter(|m| !m.tombstone) {
            let label = format!("{:03}_{}", migration.version, migration.name);
            let Some(cost) = audit_one(&extension, &label, migration.sql, HOT_TABLES) else {
                continue;
            };
            if let Some(reason) = cost.malformed.as_deref() {
                warn!(
                    migration = %cost.label(),
                    reason,
                    "Migration declares a malformed @cost directive",
                );
            }
            if cost.is_undeclared() && !applied.contains(&migration.version) {
                pending.push(cost);
            }
        }
    }
    if pending.is_empty() {
        return;
    }
    let populated = populated_hot_tables(db).await;
    for cost in pending {
        let rows: i64 = cost
            .statements
            .iter()
            .filter_map(|statement| populated.get(statement.table.as_str()))
            .copied()
            .max()
            .unwrap_or(0);
        if rows == 0 {
            continue;
        }
        warn!(
            migration = %cost.label(),
            statements = %cost.statement_summary(),
            rows,
            "Pending migration rewrites a populated hot table without a measured @cost directive",
        );
    }
}

// Why: `to_regclass` yields NULL rather than an error for a table this
// install has not created yet, and `reltuples` is -1 until the table is first
// analysed, so an unknown estimate falls back to a bounded existence probe and
// is reported as one row rather than read as empty. An estimate, not
// `count(*)`: a sequential count of four million rows would itself delay boot.
pub(super) async fn populated_hot_tables(
    db: &dyn DatabaseProvider,
) -> std::collections::HashMap<&str, i64> {
    let mut populated = std::collections::HashMap::new();
    for table in HOT_TABLES {
        let estimate = db
            .fetch_optional(
                &"SELECT reltuples::bigint AS estimate FROM pg_class WHERE oid = to_regclass($1)",
                &[table],
            )
            .await
            .ok()
            .flatten()
            .and_then(|row| row.get("estimate").and_then(serde_json::Value::as_i64));
        match estimate {
            Some(rows) if rows > 0 => {
                populated.insert(*table, rows);
            },
            Some(_) => {
                let probe = format!("SELECT 1 FROM {table} LIMIT 1");
                if db
                    .fetch_optional(&probe.as_str(), &[])
                    .await
                    .is_ok_and(|row| row.is_some())
                {
                    populated.insert(*table, 1);
                }
            },
            None => {},
        }
    }
    populated
}
