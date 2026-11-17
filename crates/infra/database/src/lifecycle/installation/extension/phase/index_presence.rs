//! Leaving an existing index's `CREATE INDEX IF NOT EXISTS` out of a phase.
//!
//! Postgres takes `SHARE` on the indexed table before it checks whether the
//! index name is taken, so re-running an index statement that is a no-op still
//! blocks every writer on that table until the phase commits, and the phase
//! takes those locks table after table in install order. A writer that holds
//! `ROW EXCLUSIVE` on one of those tables and asks for another closes a cycle
//! with it. Before each such statement the phase reads the catalog in its own
//! transaction and leaves the statement out when a relation of that name
//! already exists in the indexed table's schema, which is the exact condition
//! under which Postgres would skip it. The existing relation is kept whatever
//! its definition, as `IF NOT EXISTS` keeps it. A statement this module cannot
//! read as one non-concurrent, named `CREATE INDEX IF NOT EXISTS` is always
//! applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use pg_query::NodeEnum;

use crate::DatabaseResult;
use crate::models::DatabaseTransaction;

const INDEX_NAME_TAKEN_SQL: &str = "SELECT 1 AS taken FROM pg_class i \
     WHERE i.relname = $1 \
     AND i.relnamespace = (SELECT t.relnamespace FROM pg_class t WHERE t.oid = to_regclass($2))";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IdempotentIndex {
    name: String,
    relation: String,
}

pub(super) fn idempotent_indexes(statements: &[String]) -> HashMap<usize, IdempotentIndex> {
    statements
        .iter()
        .enumerate()
        .filter_map(|(idx, statement)| idempotent_index(statement).map(|index| (idx, index)))
        .collect()
}

fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn idempotent_index(statement: &str) -> Option<IdempotentIndex> {
    let parsed = pg_query::parse(statement).ok()?;
    let [raw] = parsed.protobuf.stmts.as_slice() else {
        return None;
    };
    let NodeEnum::IndexStmt(index) = raw.stmt.as_ref()?.node.as_ref()? else {
        return None;
    };
    if !index.if_not_exists || index.concurrent || index.idxname.is_empty() {
        return None;
    }
    let relation = index.relation.as_ref()?;
    let qualified = if relation.schemaname.is_empty() {
        quote(&relation.relname)
    } else {
        format!(
            "{}.{}",
            quote(&relation.schemaname),
            quote(&relation.relname)
        )
    };
    Some(IdempotentIndex {
        name: index.idxname.clone(),
        relation: qualified,
    })
}

pub(super) async fn name_taken(
    tx: &mut dyn DatabaseTransaction,
    index: &IdempotentIndex,
) -> DatabaseResult<bool> {
    let row = tx
        .fetch_optional(&INDEX_NAME_TAKEN_SQL, &[&index.name, &index.relation])
        .await?;
    Ok(row.is_some())
}
