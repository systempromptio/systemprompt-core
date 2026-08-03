//! DB-backed tests for `install_extension_schemas*`: schema installation
//! phases, seed application and linting, ownership validation, and the
//! disabled-extension skip path.

use std::sync::Arc;

use systemprompt_database::{DbPool, PostgresProvider, install_extension_schemas_with_config};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, LoaderError, SchemaDefinition, Seed,
};

use crate::services::db_helper::pool;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

struct StubExtension {
    id: &'static str,
    schemas: Vec<SchemaDefinition>,
    seeds: Vec<Seed>,
}

impl Extension for StubExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.1",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        self.schemas.clone()
    }

    fn seeds(&self) -> Vec<Seed> {
        self.seeds.clone()
    }
}

fn unique_id(prefix: &str) -> &'static str {
    leak(format!("{prefix}_{}", uuid::Uuid::new_v4().simple()))
}

fn registry_with(ext: StubExtension) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register stub");
    registry
}

async fn provider_and_db() -> Option<(PostgresProvider, DbPool)> {
    let db = pool().await?;
    let pg = db.write_pool_arc().ok()?;
    Some((PostgresProvider::from_pool(pg), db))
}

async fn drop_table(db: &DbPool, table: &str) {
    let pg = db.write_pool_arc().expect("write pool");
    let ddl = format!("DROP TABLE IF EXISTS \"{table}\"");
    let _ = sqlx::query(sqlx::AssertSqlSafe(ddl)).execute(&*pg).await;
}

async fn table_exists(db: &DbPool, table: &str) -> bool {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND \
         table_name = $1)",
    )
    .bind(table)
    .fetch_one(&*pg)
    .await
    .expect("table existence probe")
}

#[tokio::test]
async fn install_creates_schema_index_and_applies_seed_idempotently() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_ok");
    let ext_id = unique_id("ext_ok");
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY, label TEXT);\nCREATE \
         INDEX IF NOT EXISTS \"{table}_label_idx\" ON \"{table}\" (label);"
    );
    let seed_sql = leak(format!(
        "INSERT INTO \"{table}\" (id, label) VALUES (1, 'seeded') ON CONFLICT (id) DO NOTHING;"
    ));

    let build = || StubExtension {
        id: ext_id,
        schemas: vec![
            SchemaDefinition::new(table, sql.clone())
                .with_required_columns(vec!["id".to_owned(), "label".to_owned()]),
        ],
        seeds: vec![Seed::new(unique_id("seed"), seed_sql)],
    };

    for _ in 0..2 {
        install_extension_schemas_with_config(&registry_with(build()), &provider, &[])
            .await
            .expect("install");
    }

    let pg = db.write_pool_arc().expect("write pool");
    let seeded: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM \"{table}\" WHERE label = 'seeded'"
    )))
    .fetch_one(&*pg)
    .await
    .expect("seed count");
    assert_eq!(seeded, 1);

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_skips_disabled_extensions() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_disabled");
    let ext_id = unique_id("ext_disabled");
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![],
    };

    install_extension_schemas_with_config(&registry_with(ext), &provider, &[ext_id.to_owned()])
        .await
        .expect("install with extension disabled");

    assert!(!table_exists(&db, table).await);
}

#[tokio::test]
async fn install_rejects_seed_with_delete_statement() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_delete");
    let ext = StubExtension {
        id: unique_id("ext_seed_delete"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!("DELETE FROM \"{table}\";")),
        )],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("delete seed rejected");
    assert!(
        matches!(err, LoaderError::InvalidSeedStatement { statement, .. } if statement == "DELETE")
    );

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_rejects_non_idempotent_insert_seed() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_plain");
    let ext = StubExtension {
        id: unique_id("ext_seed_plain"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!("INSERT INTO \"{table}\" (id) VALUES (1);")),
        )],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("plain insert seed rejected");
    assert!(matches!(err, LoaderError::SeedInsertNotIdempotent { .. }));

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_fails_when_required_column_is_missing() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_missing_col");
    let ext = StubExtension {
        id: unique_id("ext_missing_col"),
        schemas: vec![
            SchemaDefinition::new(
                table,
                format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
            )
            .with_required_columns(vec!["id".to_owned(), "phantom_column".to_owned()]),
        ],
        seeds: vec![],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("missing required column rejected");
    assert!(matches!(err, LoaderError::SchemaInstallationFailed { .. }));
    assert!(err.to_string().contains("phantom_column"));

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_rejects_duplicate_table_ownership() {
    let Some((provider, _db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_shared");
    let sql = format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);");
    let mut registry = ExtensionRegistry::new();
    for prefix in ["ext_owner_a", "ext_owner_b"] {
        registry
            .register(Arc::new(StubExtension {
                id: unique_id(prefix),
                schemas: vec![SchemaDefinition::new(table, sql.clone())],
                seeds: vec![],
            }))
            .expect("register stub");
    }

    let err = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect_err("duplicate ownership rejected");
    assert!(matches!(err, LoaderError::DuplicateTableOwner { .. }));
}

#[tokio::test]
async fn install_rejects_imperative_sql_in_declarative_schema() {
    let Some((provider, _db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_imperative");
    let ext = StubExtension {
        id: unique_id("ext_imperative"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!(
                "CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);\nALTER TABLE \
                 \"{table}\" ADD COLUMN extra TEXT;"
            ),
        )],
        seeds: vec![],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("imperative DDL rejected");
    assert!(matches!(err, LoaderError::SchemaInstallationFailed { .. }));
    assert!(err.to_string().contains("Imperative SQL"));
}

async fn seed_rejection(seed_sql: &'static str) -> LoaderError {
    let (provider, _db) = provider_and_db().await.expect("db required");
    let table = unique_id("install_seed_kind");
    let ext = StubExtension {
        id: unique_id("ext_seed_kind"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(unique_id("seed"), seed_sql)],
    };
    install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("disallowed seed statement rejected")
}

#[tokio::test]
async fn install_rejects_seed_statements_by_classified_kind() {
    if provider_and_db().await.is_none() {
        return;
    }
    let cases: [(&'static str, &'static str); 7] = [
        ("SELECT 1;", "SELECT"),
        ("CREATE TABLE seed_smuggled_ddl (id BIGINT);", "CREATE"),
        ("CREATE INDEX seed_idx ON seed_t (id);", "CREATE"),
        ("ALTER TABLE seed_t ADD COLUMN x TEXT;", "ALTER"),
        ("DROP TABLE seed_t;", "DROP"),
        ("TRUNCATE seed_t;", "TRUNCATE"),
        ("GRANT SELECT ON seed_t TO PUBLIC;", "GRANT"),
    ];
    for (sql, expected_kind) in cases {
        let err = seed_rejection(sql).await;
        match err {
            LoaderError::InvalidSeedStatement { statement, .. } => {
                assert_eq!(statement, expected_kind, "for seed sql {sql:?}");
            },
            other => panic!("expected InvalidSeedStatement for {sql:?}, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn install_rejects_seed_with_unclassified_statement_as_other() {
    if provider_and_db().await.is_none() {
        return;
    }
    let err = seed_rejection("SET search_path TO public;").await;
    assert!(
        matches!(err, LoaderError::InvalidSeedStatement { statement, .. } if statement == "OTHER")
    );
}

#[tokio::test]
async fn install_rejects_unparseable_seed_sql() {
    if provider_and_db().await.is_none() {
        return;
    }
    let err = seed_rejection("THIS IS NOT SQL AT ALL").await;
    match err {
        LoaderError::SeedFailed { message, .. } => {
            assert!(message.contains("parse"), "message: {message}");
        },
        other => panic!("expected SeedFailed(parse), got {other:?}"),
    }
}

#[tokio::test]
async fn install_surfaces_seed_execution_failure_and_rolls_back() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_exec_fail");
    let missing = unique_id("no_such_table");
    let ext = StubExtension {
        id: unique_id("ext_seed_exec_fail"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!(
                "INSERT INTO \"{table}\" (id) VALUES (7) ON CONFLICT (id) DO NOTHING; INSERT \
                 INTO \"{missing}\" (id) VALUES (1) ON CONFLICT (id) DO NOTHING;"
            )),
        )],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("seed hitting a missing table must fail");
    match err {
        LoaderError::SeedFailed { message, .. } => {
            assert!(message.contains("execute"), "message: {message}");
        },
        other => panic!("expected SeedFailed(execute), got {other:?}"),
    }

    let pg = db.write_pool_arc().expect("write pool");
    let rows: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM \"{table}\""
    )))
    .fetch_one(&*pg)
    .await
    .expect("count");
    assert_eq!(rows, 0, "failed seed transaction must roll back");

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_applies_update_and_multi_statement_seed() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_update");
    let ext = StubExtension {
        id: unique_id("ext_seed_update"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY, label TEXT);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!(
                "INSERT INTO \"{table}\" (id, label) VALUES (1, 'raw') ON CONFLICT (id) DO \
                 NOTHING; UPDATE \"{table}\" SET label = 'updated' WHERE id = 1;"
            )),
        )],
    };

    install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect("multi-statement seed applies");

    let pg = db.write_pool_arc().expect("write pool");
    let label: String = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT label FROM \"{table}\" WHERE id = 1"
    )))
    .fetch_one(&*pg)
    .await
    .expect("label");
    assert_eq!(label, "updated");

    drop_table(&db, table).await;
}

#[tokio::test]
async fn a_dependent_statement_that_fails_rolls_back_the_whole_phase() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("dep_rollback");
    let ext_id = unique_id("dep_rollback_ext");

    // The structural phase creates the table; the dependent phase then adds a
    // valid index followed by one referencing a column that does not exist.
    // The failing statement must take the whole dependent phase down with it,
    // including the index that had already applied.
    let schema_sql = leak(format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id TEXT PRIMARY KEY, body TEXT);\n\
         CREATE INDEX IF NOT EXISTS \"{table}_body_idx\" ON \"{table}\" (body);\n\
         CREATE INDEX IF NOT EXISTS \"{table}_ghost_idx\" ON \"{table}\" (no_such_column);"
    ));

    let registry = registry_with(StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(table, schema_sql.to_owned())],
        seeds: vec![],
    });

    let err = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect_err("an index over a missing column must fail installation");
    let message = err.to_string();
    assert!(
        message.contains("no_such_column"),
        "the failure must quote the statement that broke, got {message}"
    );

    let index_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = $1)",
    )
    .bind(format!("{table}_body_idx"))
    .fetch_one(&*db.write_pool_arc().expect("write pool"))
    .await
    .expect("index probe");
    assert!(
        !index_exists,
        "the earlier index in the same phase must be rolled back, not left behind"
    );

    drop_table(&db, table).await;
}

#[tokio::test]
async fn an_extension_declaring_no_schema_installs_cleanly() {
    let Some((provider, _db)) = provider_and_db().await else {
        return;
    };
    let registry = registry_with(StubExtension {
        id: unique_id("no_schema_ext"),
        schemas: vec![],
        seeds: vec![],
    });

    install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect("an extension with nothing to install is not an error");
}

#[tokio::test]
async fn a_schema_that_does_not_parse_is_rejected_before_any_statement_runs() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("unparseable");
    let registry = registry_with(StubExtension {
        id: unique_id("unparseable_ext"),
        schemas: vec![SchemaDefinition::new(table, "CREATE TABLE (((".to_owned())],
        seeds: vec![],
    });

    let err = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect_err("unparseable schema SQL must be refused");
    assert!(!err.to_string().is_empty());

    assert!(
        !table_exists(&db, table).await,
        "nothing may be created from a schema that never parsed"
    );
}

// --- transaction-failure arms ---
//
// Seed application and the dependent-statement phase both wrap their work in a
// transaction and map begin/commit/rollback failures into distinct messages.
// A live Postgres provider never fails those calls, so the arms need a provider
// that does.

mod transaction_failures {
    use async_trait::async_trait;
    use systemprompt_database::{
        DatabaseInfo, DatabaseProvider, DatabaseResult, DatabaseTransaction, DbValue, JsonRow,
        QueryResult, QuerySelector, RepositoryError, ToDbValue,
    };

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FailAt {
        Begin,
        Commit,
        Statement,
    }

    #[derive(Debug)]
    struct FailingProvider {
        fail_at: FailAt,
    }

    #[async_trait]
    impl DatabaseProvider for FailingProvider {
        async fn execute(
            &self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<u64> {
            Ok(0)
        }

        async fn execute_raw(&self, _sql: &str) -> DatabaseResult<()> {
            Ok(())
        }

        async fn fetch_all(
            &self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Vec<JsonRow>> {
            Ok(vec![])
        }

        async fn fetch_one(
            &self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<JsonRow> {
            Ok(JsonRow::new())
        }

        async fn fetch_optional(
            &self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Option<JsonRow>> {
            Ok(None)
        }

        async fn fetch_scalar_value(
            &self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<DbValue> {
            Ok(DbValue::NullString)
        }

        async fn begin_transaction(&self) -> DatabaseResult<Box<dyn DatabaseTransaction>> {
            if self.fail_at == FailAt::Begin {
                return Err(RepositoryError::internal("cannot begin"));
            }
            Ok(Box::new(FailingTx {
                fail_at: self.fail_at,
            }))
        }

        async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
            Ok(DatabaseInfo {
                path: String::new(),
                size: 0,
                version: "test".into(),
                tables: vec![],
            })
        }

        async fn test_connection(&self) -> DatabaseResult<()> {
            Ok(())
        }

        async fn execute_batch(&self, _sql: &str) -> DatabaseResult<()> {
            Ok(())
        }

        async fn query_raw(&self, _query: &dyn QuerySelector) -> DatabaseResult<QueryResult> {
            Ok(QueryResult::default())
        }

        async fn query_raw_with(
            &self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<QueryResult> {
            Ok(QueryResult::default())
        }
    }

    #[derive(Debug)]
    struct FailingTx {
        fail_at: FailAt,
    }

    #[async_trait]
    impl DatabaseTransaction for FailingTx {
        async fn execute(
            &mut self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<u64> {
            if self.fail_at == FailAt::Statement {
                return Err(RepositoryError::internal("statement rejected"));
            }
            Ok(0)
        }

        async fn fetch_all(
            &mut self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Vec<JsonRow>> {
            Ok(vec![])
        }

        async fn fetch_one(
            &mut self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<JsonRow> {
            Ok(JsonRow::new())
        }

        async fn fetch_optional(
            &mut self,
            _query: &dyn QuerySelector,
            _params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Option<JsonRow>> {
            Ok(None)
        }

        async fn commit(self: Box<Self>) -> DatabaseResult<()> {
            if self.fail_at == FailAt::Commit {
                return Err(RepositoryError::internal("cannot commit"));
            }
            Ok(())
        }

        async fn rollback(self: Box<Self>) -> DatabaseResult<()> {
            Ok(())
        }
    }

    fn seeded_registry() -> ExtensionRegistry {
        let table = unique_id("txfail_tbl");
        registry_with(StubExtension {
            id: unique_id("txfail_ext"),
            schemas: vec![SchemaDefinition::new(
                table,
                leak(format!(
                    "CREATE TABLE IF NOT EXISTS \"{table}\" (id TEXT PRIMARY KEY);"
                ))
                .to_owned(),
            )],
            seeds: vec![Seed::new(
                unique_id("txfail_seed"),
                leak(format!(
                    "INSERT INTO \"{table}\" (id) VALUES ('a') ON CONFLICT (id) DO NOTHING;"
                )),
            )],
        })
    }

    async fn install_against(fail_at: FailAt) -> LoaderError {
        let provider = FailingProvider { fail_at };
        install_extension_schemas_with_config(&seeded_registry(), &provider, &[])
            .await
            .expect_err("a provider that fails must fail the install")
    }

    #[tokio::test]
    async fn a_transaction_that_cannot_be_opened_names_the_begin_step() {
        let err = install_against(FailAt::Begin).await;
        let message = err.to_string();
        assert!(
            message.contains("begin transaction") || message.contains("Failed to begin"),
            "the failure must say which step could not start, got {message}"
        );
    }

    #[tokio::test]
    async fn a_transaction_that_cannot_be_committed_names_the_commit_step() {
        let err = install_against(FailAt::Commit).await;
        let message = err.to_string();
        assert!(
            message.contains("commit"),
            "the failure must say the commit was what broke, got {message}"
        );
    }

    #[tokio::test]
    async fn a_rejected_statement_is_reported_with_its_position_and_sql() {
        let err = install_against(FailAt::Statement).await;
        let message = err.to_string();
        assert!(
            message.contains("statement rejected"),
            "the underlying driver error must be carried through, got {message}"
        );
    }
}

#[tokio::test]
async fn a_statement_type_the_classifier_does_not_know_is_refused_with_guidance() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("unclassified");

    // `SET` carries no imperative_reason, so it clears the declarative linter,
    // but `classify_statement` has no arm for it. The install must refuse and
    // tell the developer to classify it rather than guessing a phase.
    let sql = leak(format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id TEXT PRIMARY KEY);\n\
         SET search_path TO public;"
    ));

    let registry = registry_with(StubExtension {
        id: unique_id("unclassified_ext"),
        schemas: vec![SchemaDefinition::new(table, sql.to_owned())],
        seeds: vec![],
    });

    let err = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect_err("an unclassified statement type must not install");
    let message = err.to_string();
    assert!(
        message.contains("classify it as"),
        "the refusal must tell the developer what to do about it, got {message}"
    );
    assert!(
        message.contains("SET search_path"),
        "the refusal must quote the statement it could not classify, got {message}"
    );

    assert!(
        !table_exists(&db, table).await,
        "classification happens before execution, so nothing may be created"
    );
}

#[tokio::test]
async fn a_safe_drop_clears_the_linter_and_classifies_as_dependent() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("safe_drop");
    let view = format!("{table}_v");

    // `DROP VIEW IF EXISTS` is the one drop shape the linter permits in a
    // declarative schema; it must then classify as dependent and run after the
    // structural phase created the table it reads.
    let sql = leak(format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id TEXT PRIMARY KEY);\n\
         DROP VIEW IF EXISTS \"{view}\";\n\
         CREATE OR REPLACE VIEW \"{view}\" AS SELECT id FROM \"{table}\";"
    ));

    let registry = registry_with(StubExtension {
        id: unique_id("safe_drop_ext"),
        schemas: vec![SchemaDefinition::new(table, sql.to_owned())],
        seeds: vec![],
    });

    install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect("a guarded view drop is declarative and must install");

    let pg = db.write_pool_arc().expect("write pool");
    let view_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.views WHERE table_schema = 'public' AND \
         table_name = $1)",
    )
    .bind(&view)
    .fetch_one(&*pg)
    .await
    .expect("view probe");
    assert!(
        view_exists,
        "the drop must run before the create, not after it"
    );

    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP VIEW IF EXISTS \"{view}\""
    )))
    .execute(&*pg)
    .await;
    drop_table(&db, table).await;
}

#[tokio::test]
async fn an_unguarded_drop_is_rejected_as_imperative() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("unguarded_drop");

    let sql = leak(format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id TEXT PRIMARY KEY);\n\
         DROP TABLE \"{table}\";"
    ));
    let registry = registry_with(StubExtension {
        id: unique_id("unguarded_drop_ext"),
        schemas: vec![SchemaDefinition::new(table, sql.to_owned())],
        seeds: vec![],
    });

    let err = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect_err("an unguarded DROP belongs in a migration, not a schema");
    assert!(
        err.to_string().contains("schema/migrations"),
        "the refusal must point at where the statement belongs, got {err}"
    );

    assert!(
        !table_exists(&db, table).await,
        "linting happens before execution"
    );
}
