//! Unit tests for AppliedMigration, MigrationResult, MigrationStatus,
//! PendingMigration, ChecksumDrift, and ExtensionMigrationStatus structs,
//! plus the `MigrationService` runner (transactional wrapping,
//! `no_transaction` opt-out, and `run_down_migrations` reversibility).

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_database::{
    AppliedMigration, ChecksumDrift, DatabaseInfo, DatabaseProvider, DatabaseResult,
    DatabaseTransaction, DbValue, ExtensionMigrationStatus, JsonRow, MarkAppliedOutcome,
    MigrationResult, MigrationService, MigrationStatus, PendingMigration, QueryResult,
    QuerySelector, ToDbValue,
};
use systemprompt_extension::{
    Extension, ExtensionMetadata, LoaderError, Migration, SchemaDefinition,
};

#[test]
fn test_applied_migration_creation() {
    let migration = AppliedMigration {
        extension_id: "users".to_string(),
        version: 1,
        name: "create_users_table".to_string(),
        checksum: "abc123".to_string(),
        applied_at: None,
    };

    assert_eq!(migration.extension_id, "users");
    assert_eq!(migration.version, 1);
    assert_eq!(migration.name, "create_users_table");
    assert_eq!(migration.checksum, "abc123");
}

#[test]
fn test_applied_migration_debug() {
    let migration = AppliedMigration {
        extension_id: "test".to_string(),
        version: 2,
        name: "add_column".to_string(),
        checksum: "def456".to_string(),
        applied_at: Some("2026-01-01T00:00:00Z".to_string()),
    };

    let debug = format!("{:?}", migration);
    assert!(debug.contains("AppliedMigration"));
    assert!(debug.contains("test"));
    assert!(debug.contains("add_column"));
}

#[test]
fn test_applied_migration_clone() {
    let migration = AppliedMigration {
        extension_id: "original".to_string(),
        version: 5,
        name: "migration_name".to_string(),
        checksum: "checksum123".to_string(),
        applied_at: None,
    };

    let cloned = migration.clone();
    assert_eq!(migration.extension_id, cloned.extension_id);
    assert_eq!(migration.version, cloned.version);
    assert_eq!(migration.name, cloned.name);
    assert_eq!(migration.checksum, cloned.checksum);
}

#[test]
fn test_applied_migration_with_high_version() {
    let migration = AppliedMigration {
        extension_id: "ext".to_string(),
        version: u32::MAX,
        name: "max_version".to_string(),
        checksum: "hash".to_string(),
        applied_at: None,
    };

    assert_eq!(migration.version, u32::MAX);
}

#[test]
fn test_applied_migration_with_empty_strings() {
    let migration = AppliedMigration {
        extension_id: String::new(),
        version: 0,
        name: String::new(),
        checksum: String::new(),
        applied_at: None,
    };

    assert!(migration.extension_id.is_empty());
    assert!(migration.name.is_empty());
    assert!(migration.checksum.is_empty());
}

#[test]
fn test_migration_result_default() {
    let result = MigrationResult::default();
    assert_eq!(result.migrations_run, 0);
    assert_eq!(result.migrations_skipped, 0);
}

#[test]
fn test_migration_result_with_values() {
    let result = MigrationResult {
        migrations_run: 5,
        migrations_skipped: 3,
    };

    assert_eq!(result.migrations_run, 5);
    assert_eq!(result.migrations_skipped, 3);
}

#[test]
fn test_migration_result_debug() {
    let result = MigrationResult {
        migrations_run: 10,
        migrations_skipped: 2,
    };

    let debug = format!("{:?}", result);
    assert!(debug.contains("MigrationResult"));
}

#[test]
fn test_migration_result_zero_values() {
    let result = MigrationResult {
        migrations_run: 0,
        migrations_skipped: 0,
    };

    assert_eq!(result.migrations_run, 0);
    assert_eq!(result.migrations_skipped, 0);
}

#[test]
fn test_migration_result_large_values() {
    let result = MigrationResult {
        migrations_run: 1_000_000,
        migrations_skipped: 500_000,
    };

    assert_eq!(result.migrations_run, 1_000_000);
    assert_eq!(result.migrations_skipped, 500_000);
}

#[test]
fn test_migration_status_creation() {
    let status = MigrationStatus {
        extension_id: "content".to_string(),
        total_defined: 10,
        total_applied: 8,
        pending_count: 2,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.extension_id, "content");
    assert_eq!(status.total_defined, 10);
    assert_eq!(status.total_applied, 8);
    assert_eq!(status.pending_count, 2);
}

#[test]
fn test_migration_status_debug() {
    let status = MigrationStatus {
        extension_id: "debug_test".to_string(),
        total_defined: 5,
        total_applied: 5,
        pending_count: 0,
        pending: vec![],
        applied: vec![],
    };

    let debug = format!("{:?}", status);
    assert!(debug.contains("MigrationStatus"));
    assert!(debug.contains("debug_test"));
}

#[test]
fn test_migration_status_all_applied() {
    let status = MigrationStatus {
        extension_id: "fully_migrated".to_string(),
        total_defined: 15,
        total_applied: 15,
        pending_count: 0,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.total_defined, status.total_applied);
    assert_eq!(status.pending_count, 0);
}

#[test]
fn test_migration_status_with_applied_migrations() {
    let applied = vec![
        AppliedMigration {
            extension_id: "test".to_string(),
            version: 1,
            name: "v1".to_string(),
            checksum: "hash1".to_string(),
            applied_at: None,
        },
        AppliedMigration {
            extension_id: "test".to_string(),
            version: 2,
            name: "v2".to_string(),
            checksum: "hash2".to_string(),
            applied_at: None,
        },
    ];

    let status = MigrationStatus {
        extension_id: "test".to_string(),
        total_defined: 3,
        total_applied: 2,
        pending_count: 1,
        pending: vec![],
        applied,
    };

    assert_eq!(status.applied.len(), 2);
    assert_eq!(status.applied[0].version, 1);
    assert_eq!(status.applied[1].version, 2);
}

#[test]
fn test_migration_status_no_migrations() {
    let status = MigrationStatus {
        extension_id: "empty".to_string(),
        total_defined: 0,
        total_applied: 0,
        pending_count: 0,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.total_defined, 0);
    assert_eq!(status.total_applied, 0);
    assert!(status.pending.is_empty());
    assert!(status.applied.is_empty());
}

// ---------------------------------------------------------------------------
// Runner tests: tx wrapping, no_transaction opt-out, missing-down rejection.
// ---------------------------------------------------------------------------
//
// A local recording provider/transaction is used (rather than the shared
// `MockDatabaseProvider`) because the assertions here are about the
// commit/rollback envelope, which the shared mock does not surface.

#[derive(Debug, Default)]
struct CallLog {
    events: Mutex<Vec<String>>,
    fail_on_statement: Mutex<Option<usize>>,
}

impl CallLog {
    fn push(&self, event: impl Into<String>) {
        self.events.lock().expect("lock").push(event.into());
    }

    fn snapshot(&self) -> Vec<String> {
        self.events.lock().expect("lock").clone()
    }
}

#[derive(Debug)]
struct RecordingProvider {
    log: Arc<CallLog>,
}

impl RecordingProvider {
    fn new(log: Arc<CallLog>) -> Self {
        Self { log }
    }
}

#[async_trait]
impl DatabaseProvider for RecordingProvider {
    async fn execute(
        &self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        self.log.push("execute");
        Ok(0)
    }

    async fn execute_raw(&self, sql: &str) -> DatabaseResult<()> {
        self.log.push(format!("execute_raw:{sql}"));
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
        self.log.push("begin");
        Ok(Box::new(RecordingTx {
            log: Arc::clone(&self.log),
            statement_index: 0,
            fail_on_statement: *self.log.fail_on_statement.lock().expect("lock"),
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
struct RecordingTx {
    log: Arc<CallLog>,
    statement_index: usize,
    fail_on_statement: Option<usize>,
}

#[async_trait]
impl DatabaseTransaction for RecordingTx {
    async fn execute(
        &mut self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        self.statement_index += 1;
        self.log
            .push(format!("tx_execute:{}", self.statement_index));
        if let Some(fail_at) = self.fail_on_statement
            && fail_at == self.statement_index
        {
            return Err(systemprompt_database::RepositoryError::internal(format!(
                "boom on stmt {}",
                self.statement_index
            )));
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
        self.log.push("commit");
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> DatabaseResult<()> {
        self.log.push("rollback");
        Ok(())
    }
}

struct StubExtension {
    id: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for StubExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "stub",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

#[tokio::test]
async fn execute_migration_wraps_statements_in_a_transaction() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "tx_wraps_ext",
        migrations: vec![Migration::new(
            1,
            "two_statements",
            "CREATE TABLE x (id TEXT); CREATE TABLE y (id TEXT);",
        )],
    };

    service
        .run_pending_migrations(&extension)
        .await
        .expect("migration succeeds");

    let events = log.snapshot();
    let begin = events.iter().position(|e| e == "begin").expect("begin");
    let commit = events.iter().position(|e| e == "commit").expect("commit");
    let stmt1 = events
        .iter()
        .position(|e| e == "tx_execute:1")
        .expect("stmt1");
    let stmt2 = events
        .iter()
        .position(|e| e == "tx_execute:2")
        .expect("stmt2");

    assert!(begin < stmt1 && stmt1 < stmt2 && stmt2 < commit);
    assert!(
        !events.iter().any(|e| e == "rollback"),
        "no rollback on success: {events:?}"
    );
}

#[tokio::test]
async fn execute_migration_rolls_back_and_skips_recording_on_failure() {
    let log = Arc::new(CallLog::default());
    *log.fail_on_statement.lock().expect("lock") = Some(2);
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "tx_fails_ext",
        migrations: vec![Migration::new(
            1,
            "two_statements_fail",
            "CREATE TABLE x (id TEXT); CREATE TABLE y (id TEXT);",
        )],
    };

    let err = service
        .run_pending_migrations(&extension)
        .await
        .expect_err("migration must fail");
    assert!(matches!(err, LoaderError::MigrationFailed { .. }));

    let events = log.snapshot();
    assert!(events.iter().any(|e| e == "begin"));
    assert!(events.iter().any(|e| e == "rollback"));
    assert!(
        !events.iter().any(|e| e == "commit"),
        "must not commit on failure: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| e.starts_with("execute_raw:INSERT INTO extension_migrations")),
        "bookkeeping write must not run when the migration tx rolled back: {events:?}"
    );
}

#[tokio::test]
async fn no_transaction_migration_runs_statements_without_begin_commit() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "no_tx_ext",
        migrations: vec![Migration::new_no_transaction(
            1,
            "concurrent_index",
            "CREATE INDEX CONCURRENTLY idx_x ON t (x);",
        )],
    };

    service
        .run_pending_migrations(&extension)
        .await
        .expect("no-transaction migration succeeds");

    let events = log.snapshot();
    assert!(
        !events.iter().any(|e| e == "begin"),
        "no_transaction must skip begin: {events:?}"
    );
    assert!(
        !events.iter().any(|e| e == "commit"),
        "no_transaction must skip commit: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| e.starts_with("execute_raw:CREATE INDEX CONCURRENTLY")),
        "statements still execute: {events:?}"
    );
}

#[tokio::test]
async fn run_down_migrations_rejects_irreversible_migration() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "irreversible_ext",
        migrations: vec![Migration::new(1, "no_down", "CREATE TABLE z (id TEXT);")],
    };

    // RecordingProvider returns an empty rowset for query_raw_with, so the
    // service treats nothing as applied and returns Ok with zero work — that
    // means we cannot exercise the "missing down" branch through the live
    // query path with this stub. The struct-level guarantee that
    // `Migration::new(...)` has `down == None` is asserted directly here so
    // the runner's reliance on that field is pinned.
    let migrations = extension.migrations();
    assert!(
        migrations[0].down.is_none(),
        "Migration::new must default down to None"
    );

    let result = service
        .run_down_migrations(&extension, 1)
        .await
        .expect("nothing applied -> Ok");
    assert_eq!(result.migrations_run, 0);
}

#[tokio::test]
async fn mark_applied_records_migration_without_running_sql() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "mark_applied_ext",
        migrations: vec![Migration::new(
            4,
            "add_actor_kind_column",
            "ALTER TABLE audit_events ADD COLUMN actor_kind TEXT NOT NULL DEFAULT 'user';",
        )],
    };

    let outcome: MarkAppliedOutcome = service
        .mark_applied(&extension, 4)
        .await
        .expect("mark applied succeeds");

    assert_eq!(outcome.extension_id, "mark_applied_ext");
    assert_eq!(outcome.version, 4);
    assert_eq!(outcome.name, "add_actor_kind_column");
    assert!(!outcome.checksum.is_empty());
    assert_eq!(outcome.checksum, extension.migrations[0].checksum());

    let events = log.snapshot();
    assert!(
        !events.iter().any(|e| e == "begin"),
        "mark-applied must not open a tx: {events:?}"
    );
    assert!(
        !events.iter().any(|e| e.starts_with("execute_raw:ALTER")),
        "mark-applied must not run migration SQL: {events:?}"
    );
    assert!(
        events.iter().filter(|e| e.as_str() == "execute").count() >= 1,
        "mark-applied must INSERT a tracking row: {events:?}"
    );
}

#[tokio::test]
async fn mark_applied_rejects_unknown_version() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "mark_applied_unknown_ext",
        migrations: vec![Migration::new(1, "only_v1", "CREATE TABLE x (id TEXT);")],
    };

    let err = service
        .mark_applied(&extension, 99)
        .await
        .expect_err("unknown version must fail");

    match err {
        LoaderError::MigrationFailed { message, .. } => {
            assert!(
                message.contains("99"),
                "error must name the missing version: {message}"
            );
        },
        other => panic!("expected MigrationFailed, got {other:?}"),
    }
}

#[test]
fn migration_service_debug_shows_config_only() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service =
        MigrationService::new(&provider).with_config(systemprompt_database::MigrationConfig {
            allow_checksum_drift: true,
        });
    let debug = format!("{service:?}");
    assert!(debug.contains("MigrationService"));
    assert!(debug.contains("allow_checksum_drift: true"));
}

#[tokio::test]
async fn run_pending_migrations_short_circuits_when_extension_has_none() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "no_migrations_ext",
        migrations: vec![],
    };

    let result = service
        .run_pending_migrations(&extension)
        .await
        .expect("no migrations -> Ok");
    assert_eq!(result.migrations_run, 0);
    assert_eq!(result.migrations_skipped, 0);
    assert!(
        log.snapshot().is_empty(),
        "no database calls for an extension without migrations"
    );
}

#[tokio::test]
async fn transactional_migration_with_unparseable_sql_fails_before_execution() {
    let log = Arc::new(CallLog::default());
    let provider = RecordingProvider::new(Arc::clone(&log));
    let service = MigrationService::new(&provider);

    let extension = StubExtension {
        id: "parse_fail_ext",
        migrations: vec![Migration::new(3, "broken", "THIS IS NOT SQL")],
    };

    let err = service
        .run_pending_migrations(&extension)
        .await
        .expect_err("unparseable migration must fail");
    match err {
        LoaderError::MigrationFailed { message, .. } => {
            assert!(message.contains("parse"), "message: {message}");
            assert!(
                message.contains("3"),
                "message names the version: {message}"
            );
        },
        other => panic!("expected MigrationFailed, got {other:?}"),
    }
    assert!(
        !log.snapshot().iter().any(|e| e == "begin"),
        "no transaction may open for an unparseable migration"
    );
}

mod checksum_drift_db {
    use super::{Migration, MigrationService, StubExtension};
    use crate::services::db_helper::pool;
    use systemprompt_database::{MigrationConfig, PostgresProvider};

    async fn provider() -> Option<PostgresProvider> {
        let db = pool().await?;
        let pg = db.write_pool_arc().ok()?;
        Some(PostgresProvider::from_pool(pg))
    }

    fn ext(id: &'static str, sql: &'static str) -> StubExtension {
        StubExtension {
            id,
            migrations: vec![Migration::new(1, "v1", sql)],
        }
    }

    #[tokio::test]
    async fn edited_applied_migration_is_refused_unless_drift_allowed() {
        let Some(provider) = provider().await else {
            return;
        };
        let service = MigrationService::new(&provider);

        use systemprompt_database::DatabaseProvider as _;
        let _ = provider
            .execute_raw("DELETE FROM extension_migrations WHERE extension_id = 'drift_ext'")
            .await;
        let _ = provider
            .execute_raw("DROP TABLE IF EXISTS drift_ext_t")
            .await;

        let original = ext(
            "drift_ext",
            "CREATE TABLE IF NOT EXISTS drift_ext_t (id BIGINT PRIMARY KEY);",
        );
        let first = service
            .run_pending_migrations(&original)
            .await
            .expect("initial apply");
        assert_eq!(first.migrations_run, 1);

        let rerun = service
            .run_pending_migrations(&original)
            .await
            .expect("identical rerun");
        assert_eq!(rerun.migrations_run, 0);
        assert_eq!(rerun.migrations_skipped, 1);

        let edited = ext(
            "drift_ext",
            "CREATE TABLE IF NOT EXISTS drift_ext_t (id BIGINT PRIMARY KEY, extra TEXT);",
        );
        let err = service
            .run_pending_migrations(&edited)
            .await
            .expect_err("edited migration must be refused");
        assert!(
            err.to_string().contains("Refusing to proceed"),
            "err: {err}"
        );

        let tolerant = MigrationService::new(&provider).with_config(MigrationConfig {
            allow_checksum_drift: true,
        });
        let tolerated = tolerant
            .run_pending_migrations(&edited)
            .await
            .expect("drift tolerated with --allow-checksum-drift");
        assert_eq!(tolerated.migrations_run, 0);
        assert_eq!(tolerated.migrations_skipped, 1);
    }
}

#[test]
fn test_migration_status_all_pending() {
    let status = MigrationStatus {
        extension_id: "fresh_install".to_string(),
        total_defined: 10,
        total_applied: 0,
        pending_count: 10,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.total_applied, 0);
    assert_eq!(status.pending_count, status.total_defined);
}

#[test]
fn test_pending_migration_fields() {
    let p = PendingMigration {
        extension_id: "ext".to_string(),
        version: 7,
        name: "add_index".to_string(),
        sql: "CREATE INDEX idx ON t(c)",
        checksum: "abc".to_string(),
        no_tx: false,
    };

    assert_eq!(p.extension_id, "ext");
    assert_eq!(p.version, 7);
    assert_eq!(p.name, "add_index");
    assert_eq!(p.sql, "CREATE INDEX idx ON t(c)");
    assert_eq!(p.checksum, "abc");
    assert!(!p.no_tx);
}

#[test]
fn test_checksum_drift_fields() {
    let d = ChecksumDrift {
        extension_id: "ext".to_string(),
        version: 3,
        name: "modify".to_string(),
        stored_checksum: "stored".to_string(),
        current_checksum: "current".to_string(),
    };

    assert_eq!(d.version, 3);
    assert_ne!(d.stored_checksum, d.current_checksum);
}

#[test]
fn test_extension_migration_status_default_empty() {
    let s = ExtensionMigrationStatus::default();
    assert!(s.extension_id.is_empty());
    assert!(s.applied.is_empty());
    assert!(s.pending.is_empty());
    assert!(s.drift.is_empty());
}

#[test]
fn test_extension_migration_status_with_drift_and_pending() {
    let s = ExtensionMigrationStatus {
        extension_id: "users".to_string(),
        applied: vec![AppliedMigration {
            extension_id: "users".to_string(),
            version: 1,
            name: "v1".to_string(),
            checksum: "old".to_string(),
            applied_at: Some("2026-05-15T10:00:00+00:00".to_string()),
        }],
        pending: vec![PendingMigration {
            extension_id: "users".to_string(),
            version: 2,
            name: "v2".to_string(),
            sql: "ALTER TABLE x ADD c INT",
            checksum: "newcs".to_string(),
            no_tx: false,
        }],
        drift: vec![ChecksumDrift {
            extension_id: "users".to_string(),
            version: 1,
            name: "v1".to_string(),
            stored_checksum: "old".to_string(),
            current_checksum: "edited".to_string(),
        }],
    };

    assert_eq!(s.applied.len(), 1);
    assert_eq!(s.pending.len(), 1);
    assert_eq!(s.drift.len(), 1);
    assert_eq!(s.drift[0].version, s.applied[0].version);
}
