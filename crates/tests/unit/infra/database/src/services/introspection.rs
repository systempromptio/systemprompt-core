//! DB-backed tests for `services/postgres/introspection.rs`, reached through
//! [`systemprompt_database::Database::get_info`].
//!
//! `get_info` scans every table in the `public` schema and runs a per-table
//! `COUNT(*)`. Other DB-backed tests in this crate create and drop tables in
//! `public` concurrently, so introspecting the shared database would race
//! (a table listed by the catalog scan can be dropped before its `COUNT(*)`
//! runs). The test therefore spins up an isolated throwaway database, owning
//! its own `public` schema, introspects that, and drops it on the way out.

use systemprompt_database::Database;
use systemprompt_test_fixtures::fixture_database_url;

fn unique_suffix() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn swap_db_name(url: &str, new_db: &str) -> String {
    let (base, _old) = url.rsplit_once('/').expect("url has a database segment");
    format!("{base}/{new_db}")
}

#[tokio::test]
async fn get_info_reports_version_and_introspected_table() {
    let Some(admin_url) = fixture_database_url().ok() else {
        return;
    };

    let admin = match Database::new_postgres(&admin_url).await {
        Ok(db) => db,
        Err(_) => return,
    };
    let admin_pool = admin.write_pool_arc().expect("admin pool");

    let iso_db = format!("introspect_{}", unique_suffix());
    sqlx::query(&format!("CREATE DATABASE \"{iso_db}\""))
        .execute(&*admin_pool)
        .await
        .expect("create isolated database");

    let iso_url = swap_db_name(&admin_url, &iso_db);
    let result = run_introspection(&iso_url).await;

    let _ = sqlx::query(&format!(
        "DROP DATABASE IF EXISTS \"{iso_db}\" WITH (FORCE)"
    ))
    .execute(&*admin_pool)
    .await;

    result.expect("introspection assertions");
}

async fn run_introspection(iso_url: &str) -> anyhow::Result<()> {
    let db = Database::new_postgres(iso_url).await?;
    let pgpool = db.write_pool_arc().expect("iso pool");

    sqlx::query("CREATE TABLE \"widget\" (id INT PRIMARY KEY, name TEXT, flag BOOLEAN NOT NULL)")
        .execute(&*pgpool)
        .await?;
    sqlx::query("INSERT INTO \"widget\" (id, name, flag) VALUES (1, 'a', true), (2, NULL, false)")
        .execute(&*pgpool)
        .await?;

    let info = db.get_info().await?;

    anyhow::ensure!(info.path == "PostgreSQL", "path mismatch: {}", info.path);
    anyhow::ensure!(
        info.version.to_lowercase().contains("postgresql"),
        "version should name PostgreSQL, got: {}",
        info.version
    );

    let table_info = info
        .tables
        .iter()
        .find(|t| t.name == "widget")
        .ok_or_else(|| anyhow::anyhow!("introspection must surface the widget table"))?;

    anyhow::ensure!(
        table_info.row_count == 2,
        "row count must match inserted rows, got {}",
        table_info.row_count
    );

    let col_names: Vec<&str> = table_info.columns.iter().map(|c| c.name.as_str()).collect();
    anyhow::ensure!(
        col_names == ["id", "name", "flag"],
        "columns must be ordered by ordinal_position, got {col_names:?}"
    );

    let id_col = &table_info.columns[0];
    anyhow::ensure!(
        id_col.data_type == "integer",
        "id type: {}",
        id_col.data_type
    );
    anyhow::ensure!(!id_col.nullable, "PRIMARY KEY column is NOT NULL");

    anyhow::ensure!(
        table_info.columns[1].nullable,
        "name has no NOT NULL constraint"
    );

    let flag_col = &table_info.columns[2];
    anyhow::ensure!(
        flag_col.data_type == "boolean",
        "flag type: {}",
        flag_col.data_type
    );
    anyhow::ensure!(!flag_col.nullable, "flag is NOT NULL");

    Ok(())
}
