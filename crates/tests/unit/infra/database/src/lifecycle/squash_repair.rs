//! Tests for `SquashPlan`, `RepairResult`, and squash range validation logic.
//!
//! The squash/repair helpers need a `MigrationService`, which in turn needs a
//! `DatabaseProvider`. We reuse a minimal recording stub so we can drive the
//! pure-logic paths (range validation, baseline SQL assembly, checksum
//! calculation) without a live database.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_database::{
    AppliedMigration, DatabaseInfo, DatabaseProvider, DatabaseResult, DatabaseTransaction, DbValue,
    JsonRow, MigrationService, QueryResult, QuerySelector, RepairResult, SquashPlan, ToDbValue,
};
use systemprompt_extension::{Extension, ExtensionMetadata, Migration, SchemaDefinition};

#[derive(Debug, Default)]
struct AppliedRows {
    rows: Vec<(u32, String)>,
}

#[derive(Debug, Default)]
struct SilentProvider {
    applied: Mutex<AppliedRows>,
    execute_log: Mutex<Vec<String>>,
}

impl SilentProvider {
    #[allow(dead_code)]
    fn with_applied(versions: &[(u32, &str)]) -> Self {
        let rows = versions.iter().map(|(v, n)| (*v, n.to_string())).collect();
        Self {
            applied: Mutex::new(AppliedRows { rows }),
            execute_log: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl DatabaseProvider for SilentProvider {
    async fn execute(
        &self,
        query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        self.execute_log
            .lock()
            .expect("lock")
            .push(query.select_query().to_string());
        Ok(1)
    }

    async fn execute_raw(&self, sql: &str) -> DatabaseResult<()> {
        self.execute_log.lock().expect("lock").push(sql.to_string());
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
        Ok(Box::new(SilentTx {
            log: Arc::new(Mutex::new(vec![])),
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
        query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<QueryResult> {
        let sql = query.select_query();
        if sql.contains("extension_migrations") && sql.contains("SELECT") {
            let applied = self.applied.lock().expect("lock");
            if applied.rows.is_empty() {
                return Ok(QueryResult::default());
            }
            let rows: Vec<JsonRow> = applied
                .rows
                .iter()
                .map(|(version, name)| {
                    let mut row = JsonRow::new();
                    row.insert(
                        "extension_id".to_string(),
                        serde_json::Value::String("test_ext".to_string()),
                    );
                    row.insert(
                        "version".to_string(),
                        serde_json::Value::Number((*version as i64).into()),
                    );
                    row.insert("name".to_string(), serde_json::Value::String(name.clone()));
                    row.insert(
                        "checksum".to_string(),
                        serde_json::Value::String("aabbcc".to_string()),
                    );
                    row.insert("applied_at".to_string(), serde_json::Value::Null);
                    row
                })
                .collect();

            return Ok(QueryResult {
                columns: vec![
                    "extension_id".to_string(),
                    "version".to_string(),
                    "name".to_string(),
                    "checksum".to_string(),
                    "applied_at".to_string(),
                ],
                row_count: rows.len(),
                rows,
                execution_time_ms: 0,
            });
        }
        Ok(QueryResult::default())
    }
}

#[derive(Debug)]
struct SilentTx {
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl DatabaseTransaction for SilentTx {
    async fn execute(
        &mut self,
        query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        self.log
            .lock()
            .expect("lock")
            .push(query.select_query().to_string());
        Ok(1)
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
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> DatabaseResult<()> {
        Ok(())
    }
}

struct StubExt {
    id: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for StubExt {
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

#[test]
fn squash_plan_fields_are_accessible() {
    let plan = SquashPlan {
        extension_id: "ext".to_string(),
        through: 3,
        baseline_name: "baseline_v3".to_string(),
        baseline_sql: "-- combined\n".to_string(),
        baseline_checksum: "abc123".to_string(),
        source_versions: vec![1, 2, 3],
        already_applied_versions: vec![1, 2, 3],
        applied: false,
    };

    assert_eq!(plan.extension_id, "ext");
    assert_eq!(plan.through, 3);
    assert_eq!(plan.source_versions.len(), 3);
    assert!(!plan.applied);
}

#[test]
fn squash_plan_applied_flag_true() {
    let plan = SquashPlan {
        extension_id: "ext".to_string(),
        through: 2,
        baseline_name: "baseline_v2".to_string(),
        baseline_sql: "SQL".to_string(),
        baseline_checksum: "x".to_string(),
        source_versions: vec![1, 2],
        already_applied_versions: vec![1, 2],
        applied: true,
    };
    assert!(plan.applied);
}

#[test]
fn squash_plan_debug() {
    let plan = SquashPlan {
        extension_id: "e".to_string(),
        through: 1,
        baseline_name: "b".to_string(),
        baseline_sql: "s".to_string(),
        baseline_checksum: "c".to_string(),
        source_versions: vec![1],
        already_applied_versions: vec![1],
        applied: false,
    };
    let debug = format!("{:?}", plan);
    assert!(debug.contains("SquashPlan"));
}

#[test]
fn squash_plan_clone() {
    let plan = SquashPlan {
        extension_id: "ext".to_string(),
        through: 5,
        baseline_name: "baseline_v5".to_string(),
        baseline_sql: "combined SQL".to_string(),
        baseline_checksum: "deadbeef".to_string(),
        source_versions: vec![1, 2, 3, 4, 5],
        already_applied_versions: vec![1, 2, 3, 4, 5],
        applied: true,
    };
    let cloned = plan.clone();
    assert_eq!(plan.extension_id, cloned.extension_id);
    assert_eq!(plan.through, cloned.through);
    assert_eq!(plan.source_versions, cloned.source_versions);
    assert_eq!(plan.applied, cloned.applied);
}

#[test]
fn repair_result_default_is_empty() {
    let result = RepairResult::default();
    assert!(result.repaired.is_empty());
    assert_eq!(result.migrations_run, 0);
}

#[test]
fn repair_result_with_data() {
    let drift = systemprompt_database::ChecksumDrift {
        extension_id: "ext".to_string(),
        version: 2,
        name: "mod".to_string(),
        stored_checksum: "old".to_string(),
        current_checksum: "new".to_string(),
    };
    let result = RepairResult {
        repaired: vec![drift],
        migrations_run: 1,
    };

    assert_eq!(result.repaired.len(), 1);
    assert_eq!(result.migrations_run, 1);
}

#[test]
fn repair_result_debug() {
    let debug = format!("{:?}", RepairResult::default());
    assert!(debug.contains("RepairResult"));
}

#[test]
fn repair_result_clone() {
    let result = RepairResult {
        repaired: vec![],
        migrations_run: 3,
    };
    let cloned = result.clone();
    assert_eq!(result.migrations_run, cloned.migrations_run);
}

#[tokio::test]
async fn squash_through_rejects_zero_version() {
    let provider = SilentProvider::default();
    let service = MigrationService::new(&provider);

    let ext = StubExt {
        id: "squash_zero",
        migrations: vec![Migration::new(1, "v1", "CREATE TABLE x (id TEXT);")],
    };

    let result = service.squash_through(&ext, 0, false).await;
    assert!(result.is_err(), "through=0 must be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("0") || msg.contains("version 0"),
        "error: {msg}"
    );
}

#[tokio::test]
async fn squash_through_rejects_missing_range() {
    let provider = SilentProvider::default();
    let service = MigrationService::new(&provider);

    let ext = StubExt {
        id: "squash_missing",
        migrations: vec![
            Migration::new(1, "v1", "CREATE TABLE x (id TEXT);"),
            Migration::new(3, "v3", "CREATE TABLE y (id TEXT);"),
        ],
    };

    let result = service.squash_through(&ext, 3, false).await;
    assert!(
        result.is_err(),
        "non-contiguous range 1,3 must be rejected for through=3"
    );
}

#[tokio::test]
async fn squash_through_rejects_no_defined_migrations_in_range() {
    let provider = SilentProvider::default();
    let service = MigrationService::new(&provider);

    let ext = StubExt {
        id: "squash_empty",
        migrations: vec![Migration::new(5, "v5", "CREATE TABLE z (id TEXT);")],
    };

    let result = service.squash_through(&ext, 3, false).await;
    assert!(result.is_err(), "no migrations in 1..=3 must be rejected");
}

#[tokio::test]
async fn repair_drift_returns_empty_when_no_drift() {
    let provider = SilentProvider::default();
    let service = MigrationService::new(&provider);

    let ext = StubExt {
        id: "no_drift_ext",
        migrations: vec![Migration::new(1, "v1", "CREATE TABLE t (id TEXT);")],
    };

    let result = service.repair_drift(&ext).await.expect("repair ok");
    assert!(result.repaired.is_empty());
    assert_eq!(result.migrations_run, 0);
}

#[test]
fn migration_config_default_drift_not_allowed() {
    let cfg = systemprompt_database::MigrationConfig::default();
    assert!(!cfg.allow_checksum_drift);
}

#[test]
fn applied_migration_fields() {
    let m = AppliedMigration {
        extension_id: "users".to_string(),
        version: 7,
        name: "add_email".to_string(),
        checksum: "abc".to_string(),
        applied_at: Some("2026-01-01T00:00:00Z".to_string()),
    };
    assert_eq!(m.version, 7);
    assert!(m.applied_at.is_some());
}
