use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use systemprompt_analytics::projection::{
    ReportingProjector, ReportingRow, ReportingSource, SOURCE_DEFINITIONS, heartbeat_rebuild,
    write_snapshot_page,
};

async fn isolated_projection() -> Transaction<'static, Postgres> {
    let url = systemprompt_test_fixtures::fixture_database_url().expect("PostgreSQL test URL");
    let pool = PgPool::connect(&url).await.expect("PostgreSQL connection");
    let mut tx = pool.begin().await.expect("transaction");
    let schema = format!("projection_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT set_config('search_path', $1, true)")
        .bind(&schema)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(analytics_schema_sql(|table| {
        table.starts_with("analytics_projection_") || table.starts_with("analytics_report_")
    })))
    .execute(&mut *tx)
    .await
    .unwrap();
    for script in [
        include_str!("../../../../../domain/users/schema/users.sql"),
        include_str!("../../../../../domain/analytics/schema/reporting_privacy.sql"),
    ] {
        let script = script
            .replace("public.", &format!("{schema}."))
            .replace("pg_catalog, public", &format!("pg_catalog, {schema}"));
        sqlx::raw_sql(sqlx::AssertSqlSafe(script))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    let capture = include_str!("../../../../../domain/users/schema/reporting_capture.sql");
    let view = capture
        .split("CREATE OR REPLACE TRIGGER")
        .next()
        .expect("users reporting view precedes its trigger");
    sqlx::raw_sql(sqlx::AssertSqlSafe(view.to_owned()))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,name,email) VALUES ('u','baseline','u@example.test'), ('replacement','replacement','replacement@example.test')")
        .execute(&mut *tx).await.unwrap();
    sqlx::query(systemprompt_analytics::projection::REPORTING_STATE_SEED)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx
}

fn user(key: &str, revision: i64, name: &str) -> ReportingRow {
    ReportingRow {
        source: ReportingSource::Users,
        key: key.into(),
        revision,
        deleted: false,
        row: json!({"id": key, "name": name, "status": "active", "roles": ["user"], "created_at": "2026-01-01T00:00:00Z"}),
    }
}

/// Opens a generation at `cutoff`, clears the targets and pages the users
/// source in one-row pages, returning the generation and the keys paged.
async fn snapshot_users(tx: &mut Transaction<'_, Postgres>, cutoff: i64) -> (i64, Vec<String>) {
    let generation = ReportingProjector::begin_rebuild(&mut *tx, cutoff)
        .await
        .unwrap();
    ReportingProjector::clear_targets(&mut *tx, generation)
        .await
        .unwrap();
    let users = ReportingSource::Users.definition();
    let mut after: Option<String> = None;
    let mut keys = Vec::new();
    loop {
        let page = write_snapshot_page(&mut *tx, users, after.as_deref(), 1)
            .await
            .unwrap();
        heartbeat_rebuild(&mut *tx, generation, users.table, page.written)
            .await
            .unwrap();
        if page.fetched < 1 {
            break;
        }
        assert_eq!((page.fetched, page.written), (1, 1));
        after = page.last_key;
        keys.push(after.clone().unwrap());
    }
    (generation, keys)
}

async fn name(tx: &mut Transaction<'_, Postgres>, key: &str) -> Option<String> {
    sqlx::query_scalar("SELECT name FROM analytics_report_users WHERE id = $1")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await
        .unwrap()
}

#[tokio::test]
async fn revisions_tombstones_and_baseline_prevent_resurrection() {
    let mut tx = isolated_projection().await;
    assert!(
        ReportingProjector::apply_fact(&mut tx, &user("u", 1, "early"))
            .await
            .is_err()
    );
    let (generation, keys) = snapshot_users(&mut tx, 10).await;
    assert_eq!(keys, vec!["replacement".to_owned(), "u".to_owned()]);
    ReportingProjector::finish_rebuild(&mut tx, generation)
        .await
        .unwrap();
    assert!(
        heartbeat_rebuild(&mut tx, generation, "users", 0)
            .await
            .unwrap_err()
            .is_rebuild_superseded()
    );
    assert!(
        !ReportingProjector::apply_fact(&mut tx, &user("u", 9, "stale"))
            .await
            .unwrap()
    );
    assert!(
        !ReportingProjector::apply_fact(&mut tx, &user("absent", 10, "stale"))
            .await
            .unwrap()
    );
    assert_eq!(name(&mut tx, "u").await.as_deref(), Some("baseline"));
    assert!(name(&mut tx, "absent").await.is_none());
    assert!(
        ReportingProjector::apply_fact(&mut tx, &user("u", 12, "new"))
            .await
            .unwrap()
    );
    assert!(
        !ReportingProjector::apply_fact(&mut tx, &user("u", 12, "duplicate"))
            .await
            .unwrap()
    );
    assert!(
        !ReportingProjector::apply_fact(&mut tx, &user("u", 11, "late commit"))
            .await
            .unwrap()
    );
    assert_eq!(name(&mut tx, "u").await.as_deref(), Some("new"));
    let deletion = ReportingRow {
        deleted: true,
        row: Value::Null,
        ..user("u", 13, "")
    };
    assert!(
        ReportingProjector::apply_fact(&mut tx, &deletion)
            .await
            .unwrap()
    );
    assert!(name(&mut tx, "u").await.is_none());
    assert!(
        !ReportingProjector::apply_fact(&mut tx, &user("u", 12, "resurrection"))
            .await
            .unwrap()
    );
    assert!(
        ReportingProjector::apply_fact(&mut tx, &user("u", 14, "recreated"))
            .await
            .unwrap()
    );
    assert_eq!(name(&mut tx, "u").await.as_deref(), Some("recreated"));
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn rebuild_primitives_roll_back_and_fence_and_validate_contracts() {
    let mut tx = isolated_projection().await;
    let (generation, _) = snapshot_users(&mut tx, 10).await;
    ReportingProjector::finish_rebuild(&mut tx, generation)
        .await
        .unwrap();
    assert_eq!(name(&mut tx, "u").await.as_deref(), Some("baseline"));
    sqlx::query("SAVEPOINT rebuild")
        .execute(&mut *tx)
        .await
        .unwrap();
    let generation = ReportingProjector::begin_rebuild(&mut tx, 20)
        .await
        .unwrap();
    ReportingProjector::clear_targets(&mut tx, generation)
        .await
        .unwrap();
    assert!(name(&mut tx, "u").await.is_none());
    assert!(
        ReportingProjector::clear_targets(&mut tx, generation - 1)
            .await
            .unwrap_err()
            .is_rebuild_superseded()
    );
    assert!(
        ReportingProjector::finish_rebuild(&mut tx, generation - 1)
            .await
            .unwrap_err()
            .is_rebuild_superseded()
    );
    sqlx::query("ROLLBACK TO SAVEPOINT rebuild")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(name(&mut tx, "u").await.as_deref(), Some("baseline"));
    assert_eq!(generation, 2);
    let mut mismatch = user("u", 11, "invalid");
    mismatch.row["id"] = json!("different");
    assert!(
        ReportingProjector::apply_fact(&mut tx, &mismatch)
            .await
            .is_err()
    );
    mismatch = user("u", 11, "invalid");
    mismatch.row["secret"] = json!("unexpected field");
    assert!(
        ReportingProjector::apply_fact(&mut tx, &mismatch)
            .await
            .is_err()
    );
    for definition in SOURCE_DEFINITIONS {
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT attname::text FROM pg_attribute WHERE attrelid = $1::regclass AND attnum > 0 AND NOT attisdropped ORDER BY attnum",
        ).bind(definition.target).fetch_all(&mut *tx).await.unwrap();
        assert_eq!(columns, definition.columns, "{} contract", definition.table);
    }
    tx.rollback().await.unwrap();
}

fn analytics_schema_sql(select: impl Fn(&str) -> bool) -> String {
    use systemprompt_extension::Extension;
    systemprompt_analytics::AnalyticsExtension
        .schemas()
        .into_iter()
        .filter(|schema| schema.table.as_deref().is_some_and(&select))
        .map(|schema| schema.sql)
        .collect::<Vec<_>>()
        .join("\n")
}
