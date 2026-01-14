use anyhow::Result;
use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::models::{ColumnInfo, DatabaseInfo, TableInfo};

pub async fn get_database_info(pool: &PgPool) -> Result<DatabaseInfo> {
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

        let quoted_table = quote_identifier(&table_name);
        let count_query = format!("SELECT COUNT(*) as count FROM {quoted_table}");
        let count_row = sqlx::query(&count_query).fetch_one(pool).await?;
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
        path: "PostgreSQL".to_string(),
        size: 0,
        version,
        tables,
    })
}

fn quote_identifier(identifier: &str) -> String {
    let escaped = identifier.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
