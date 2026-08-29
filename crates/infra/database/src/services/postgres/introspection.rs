//! `PostgreSQL` schema introspection used by [`crate::Database::get_info`].
//!
//! Part of the documented sqlx allowlist — the queries against
//! `information_schema` are dynamic by design: per-table `SELECT COUNT(*)`
//! statements have to be built at runtime against runtime-supplied table
//! names, and the result columns are typed dynamically.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::Row;
use sqlx::postgres::PgPool;

use crate::admin::SafeIdentifier;
use crate::error::DatabaseResult;
use crate::models::{ColumnInfo, DatabaseInfo, TableInfo};

pub(super) async fn get_database_info(pool: &PgPool) -> DatabaseResult<DatabaseInfo> {
    let version_row = sqlx::query("SELECT version() as version")
        .fetch_one(pool)
        .await?;
    let version: String = version_row.try_get("version")?;

    let table_rows = sqlx::query(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' ORDER BY \
         table_name",
    )
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::new();
    for table_row in table_rows {
        let table_name: String = table_row.try_get("table_name")?;

        // Why: the name comes from `information_schema`, but provenance is not
        // a property the type system can check and the next caller of this
        // function may not have it. Validate before interpolating; a catalog
        // name that cannot be a plain identifier is skipped and named rather
        // than quoted and hoped for.
        let Ok(safe_table) = SafeIdentifier::parse(&table_name) else {
            tracing::warn!(
                table = %table_name,
                "skipping table whose name is not a plain identifier"
            );
            continue;
        };
        let quoted_table = safe_table.quoted();
        let count_query = format!("SELECT COUNT(*) as count FROM {quoted_table}");
        // Why: the table list and the per-table count are separate queries, so
        // a table dropped in between (a concurrent migration) yields 42P01;
        // skip the vanished table instead of failing the whole introspection.
        let count_row = match sqlx::query(sqlx::AssertSqlSafe(count_query))
            .fetch_one(pool)
            .await
        {
            Ok(row) => row,
            Err(e) if is_undefined_table(&e) => continue,
            Err(e) => return Err(e.into()),
        };
        let row_count: i64 = count_row.try_get("count")?;

        let column_rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable FROM information_schema.columns WHERE \
             table_name = $1 ORDER BY ordinal_position",
        )
        .bind(&table_name)
        .fetch_all(pool)
        .await?;

        let mut columns = Vec::new();
        for col_row in column_rows {
            let col_name: String = col_row.try_get("column_name")?;
            let col_type: String = col_row.try_get("data_type")?;
            let is_nullable: String = col_row.try_get("is_nullable")?;

            columns.push(ColumnInfo {
                name: col_name,
                data_type: col_type,
                nullable: is_nullable == "YES",
                primary_key: false,
                default: None,
            });
        }

        tables.push(TableInfo {
            name: table_name,
            row_count,
            size_bytes: 0,
            columns,
        });
    }

    Ok(DatabaseInfo {
        path: "PostgreSQL".to_owned(),
        size: 0,
        version,
        tables,
    })
}

fn is_undefined_table(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "42P01")
}
