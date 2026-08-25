use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_users::MERGE_EXCLUDED_SECURITY_TABLES;

#[tokio::test]
async fn excluded_security_tables_cascade_from_users() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let pg = pool.pool_arc().expect("pg pool");

    for table in MERGE_EXCLUDED_SECURITY_TABLES {
        let rule: Option<String> = sqlx::query_scalar(
            r"SELECT rc.delete_rule
              FROM information_schema.table_constraints tc
              JOIN information_schema.referential_constraints rc
                ON rc.constraint_name = tc.constraint_name
               AND rc.constraint_schema = tc.constraint_schema
              JOIN information_schema.constraint_column_usage ccu
                ON ccu.constraint_name = tc.constraint_name
               AND ccu.constraint_schema = tc.constraint_schema
              WHERE tc.table_name = $1
                AND tc.constraint_type = 'FOREIGN KEY'
                AND ccu.table_name = 'users'
              LIMIT 1",
        )
        .bind(table)
        .fetch_optional(pg.as_ref())
        .await
        .expect("query")
        .flatten();

        assert_eq!(
            rule.as_deref(),
            Some("CASCADE"),
            "{table} is listed in MERGE_EXCLUDED_SECURITY_TABLES, which relies on its rows dying \
             with the source user, but it has no ON DELETE CASCADE foreign key to users"
        );
    }
}
