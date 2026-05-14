//! Compare declared columns against the live database and emit additive
//! `ALTER TABLE` statements for any column missing from a pre-existing table.

use super::{DeclaredColumn, DeclaredTable};
use crate::error::DatabaseResult;
use crate::services::provider::DatabaseProvider;

/// Compare declared columns against the live database and emit additive
/// `ALTER TABLE` statements for any column missing from a pre-existing table.
///
/// Returns `ALTER TABLE … ADD COLUMN IF NOT EXISTS …` statements for every
/// declared column not present live. Tables that do not exist live are skipped
/// — `CREATE TABLE IF NOT EXISTS` will create them.
pub async fn compute_additive_alters(
    db: &dyn DatabaseProvider,
    tables: &[DeclaredTable],
) -> DatabaseResult<Vec<String>> {
    let mut out = Vec::new();
    for table in tables {
        let live = live_columns(db, &table.name).await?;
        if live.is_empty() {
            continue;
        }
        let missing: Vec<&DeclaredColumn> = table
            .columns
            .iter()
            .filter(|c| !live.iter().any(|lc| lc.eq_ignore_ascii_case(&c.name)))
            .collect();
        if missing.is_empty() {
            continue;
        }
        let mut sql = format!("ALTER TABLE {} ", quote_ident(&table.name));
        for (idx, col) in missing.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            sql.push_str("ADD COLUMN IF NOT EXISTS ");
            sql.push_str(&quote_ident(&col.name));
            sql.push(' ');
            sql.push_str(&col.type_text);
        }
        out.push(sql);
    }
    Ok(out)
}

async fn live_columns(db: &dyn DatabaseProvider, table: &str) -> DatabaseResult<Vec<String>> {
    let result = db
        .query_raw_with(
            &"SELECT column_name FROM information_schema.columns WHERE table_schema = 'public' \
              AND table_name = $1",
            vec![serde_json::Value::String(table.to_string())],
        )
        .await?;
    let mut names = Vec::with_capacity(result.rows.len());
    for row in result.rows {
        if let Some(serde_json::Value::String(n)) = row.get("column_name") {
            names.push(n.clone());
        }
    }
    Ok(names)
}

fn quote_ident(ident: &str) -> String {
    let safe = ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if safe && !ident.is_empty() && !ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.to_string()
    } else {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }
}
