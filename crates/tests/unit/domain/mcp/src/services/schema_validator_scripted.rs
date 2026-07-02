//! Scripted-provider tests for [`SchemaValidator`] covering the table-exists,
//! column-validation, auto-migrate, and strict-failure paths without a live
//! database.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use systemprompt_database::{
    DatabaseInfo, DatabaseProvider, DatabaseResult, DatabaseTransaction, DbValue, JsonRow,
    QueryResult, QuerySelector, RepositoryError, ToDbValue,
};
use systemprompt_mcp::services::schema::{SchemaValidationMode, SchemaValidator};
use systemprompt_models::mcp::deployment::SchemaDefinition;

struct ScriptedProvider {
    tables: Mutex<HashSet<String>>,
    columns: Mutex<HashMap<String, Vec<String>>>,
    executed: Mutex<Vec<String>>,
    create_adds_table: Option<(String, Vec<String>)>,
}

impl ScriptedProvider {
    fn new(tables: &[&str], columns: &[(&str, &[&str])]) -> Self {
        Self {
            tables: Mutex::new(tables.iter().map(|s| (*s).to_owned()).collect()),
            columns: Mutex::new(
                columns
                    .iter()
                    .map(|(t, cols)| {
                        (
                            (*t).to_owned(),
                            cols.iter().map(|c| (*c).to_owned()).collect(),
                        )
                    })
                    .collect(),
            ),
            executed: Mutex::new(Vec::new()),
            create_adds_table: None,
        }
    }

    fn with_create_result(mut self, table: &str, cols: &[&str]) -> Self {
        self.create_adds_table = Some((
            table.to_owned(),
            cols.iter().map(|c| (*c).to_owned()).collect(),
        ));
        self
    }
}

impl std::fmt::Debug for ScriptedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ScriptedProvider")
    }
}

#[async_trait::async_trait]
impl DatabaseProvider for ScriptedProvider {
    async fn execute(&self, q: &dyn QuerySelector, _p: &[&dyn ToDbValue]) -> DatabaseResult<u64> {
        self.executed
            .lock()
            .unwrap()
            .push(q.select_query().to_owned());
        if let Some((table, cols)) = &self.create_adds_table {
            self.tables.lock().unwrap().insert(table.clone());
            self.columns
                .lock()
                .unwrap()
                .insert(table.clone(), cols.clone());
        }
        Ok(0)
    }

    async fn execute_raw(&self, _sql: &str) -> DatabaseResult<()> {
        Ok(())
    }

    async fn fetch_all(
        &self,
        q: &dyn QuerySelector,
        _p: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>> {
        let sql = q.select_query().to_owned();
        let columns = self.columns.lock().unwrap();
        for (table, cols) in columns.iter() {
            if sql.contains(table.as_str()) {
                return Ok(cols
                    .iter()
                    .map(|c| {
                        let mut row = JsonRow::new();
                        row.insert("name".to_owned(), serde_json::Value::String(c.clone()));
                        row
                    })
                    .collect());
            }
        }
        Ok(vec![])
    }

    async fn fetch_one(
        &self,
        _q: &dyn QuerySelector,
        _p: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow> {
        Err(RepositoryError::not_found("no row"))
    }

    async fn fetch_optional(
        &self,
        _q: &dyn QuerySelector,
        p: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>> {
        let requested = p
            .first()
            .map(|v| format!("{:?}", v.to_db_value()))
            .unwrap_or_default();
        let tables = self.tables.lock().unwrap();
        if tables.iter().any(|t| requested.contains(t.as_str())) {
            let mut row = JsonRow::new();
            row.insert("name".to_owned(), serde_json::Value::String("t".to_owned()));
            return Ok(Some(row));
        }
        Ok(None)
    }

    async fn fetch_scalar_value(
        &self,
        _q: &dyn QuerySelector,
        _p: &[&dyn ToDbValue],
    ) -> DatabaseResult<DbValue> {
        Ok(DbValue::NullString)
    }

    async fn begin_transaction(&self) -> DatabaseResult<Box<dyn DatabaseTransaction>> {
        Err(RepositoryError::not_found("no tx"))
    }

    async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
        Err(RepositoryError::not_found("no info"))
    }

    async fn test_connection(&self) -> DatabaseResult<()> {
        Ok(())
    }

    async fn execute_batch(&self, _sql: &str) -> DatabaseResult<()> {
        Ok(())
    }

    async fn query_raw(&self, _q: &dyn QuerySelector) -> DatabaseResult<QueryResult> {
        Ok(QueryResult::default())
    }

    async fn query_raw_with(
        &self,
        _q: &dyn QuerySelector,
        _p: &[&dyn ToDbValue],
    ) -> DatabaseResult<QueryResult> {
        Ok(QueryResult::default())
    }
}

fn schema_def(table: &str, file: &str, cols: &[&str]) -> SchemaDefinition {
    SchemaDefinition {
        table: table.to_owned(),
        file: file.to_owned(),
        required_columns: cols.iter().map(|c| (*c).to_owned()).collect(),
    }
}

#[tokio::test]
async fn existing_table_with_required_columns_validates() {
    let db = ScriptedProvider::new(&["users"], &[("users", &["id", "email"])]);
    let validator = SchemaValidator::new(&db, SchemaValidationMode::AutoMigrate);

    let report = validator
        .validate_and_apply(
            "svc",
            std::path::Path::new("/tmp"),
            &[schema_def("users", "users.sql", &["id", "email"])],
        )
        .await
        .expect("validation should pass");

    assert_eq!(report.validated, 1);
    assert_eq!(report.created, 0);
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

#[tokio::test]
async fn existing_table_missing_column_warns_in_automigrate() {
    let db = ScriptedProvider::new(&["users"], &[("users", &["id"])]);
    let validator = SchemaValidator::new(&db, SchemaValidationMode::AutoMigrate);

    let report = validator
        .validate_and_apply(
            "svc",
            std::path::Path::new("/tmp"),
            &[schema_def("users", "users.sql", &["id", "email"])],
        )
        .await
        .expect("automigrate downgrades to warning");

    assert_eq!(report.validated, 0);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("missing required columns"));
    assert!(report.warnings[0].contains("email"));
}

#[tokio::test]
async fn existing_table_missing_column_fails_in_strict() {
    let db = ScriptedProvider::new(&["users"], &[("users", &["id"])]);
    let validator = SchemaValidator::new(&db, SchemaValidationMode::Strict);

    let err = validator
        .validate_and_apply(
            "svc",
            std::path::Path::new("/tmp"),
            &[schema_def("users", "users.sql", &["id", "email"])],
        )
        .await
        .expect_err("strict mode must fail");

    assert!(err.to_string().contains("users"));
}

#[tokio::test]
async fn missing_table_in_strict_mode_fails() {
    let db = ScriptedProvider::new(&[], &[]);
    let validator = SchemaValidator::new(&db, SchemaValidationMode::Strict);

    let err = validator
        .validate_and_apply(
            "svc",
            std::path::Path::new("/tmp"),
            &[schema_def("ghost", "ghost.sql", &["id"])],
        )
        .await
        .expect_err("missing table without automigrate must fail");

    assert!(err.to_string().contains("does not exist"));
}

#[tokio::test]
async fn automigrate_creates_missing_table_from_schema_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("items.sql"),
        "CREATE TABLE items (id TEXT PRIMARY KEY, label TEXT);",
    )
    .expect("write schema file");

    let db = ScriptedProvider::new(&[], &[]).with_create_result("items", &["id", "label"]);
    let validator = SchemaValidator::new(&db, SchemaValidationMode::AutoMigrate);

    let report = validator
        .validate_and_apply(
            "svc",
            dir.path(),
            &[schema_def("items", "items.sql", &["id", "label"])],
        )
        .await
        .expect("automigrate should create table");

    assert_eq!(report.validated, 1);
    assert_eq!(report.created, 1);
    assert!(report.errors.is_empty());
    assert!(
        db.executed
            .lock()
            .unwrap()
            .iter()
            .any(|sql| sql.contains("CREATE TABLE items"))
    );
}

#[tokio::test]
async fn automigrate_create_that_leaves_no_table_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("items.sql"),
        "CREATE TABLE items (id TEXT PRIMARY KEY);",
    )
    .expect("write schema file");

    let db = ScriptedProvider::new(&[], &[]);
    let validator = SchemaValidator::new(&db, SchemaValidationMode::AutoMigrate);

    let report = validator
        .validate_and_apply(
            "svc",
            dir.path(),
            &[schema_def("items", "items.sql", &["id"])],
        )
        .await
        .expect("automigrate downgrades to warning");

    assert_eq!(report.validated, 0);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("was not created"));
}

#[tokio::test]
async fn automigrate_missing_schema_file_warns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = ScriptedProvider::new(&[], &[]);
    let validator = SchemaValidator::new(&db, SchemaValidationMode::AutoMigrate);

    let report = validator
        .validate_and_apply(
            "svc",
            dir.path(),
            &[schema_def("items", "missing.sql", &["id"])],
        )
        .await
        .expect("automigrate downgrades to warning");

    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("Failed to load schema file"));
}

#[tokio::test]
async fn invalid_table_identifier_is_rejected_before_pragma() {
    let db = ScriptedProvider::new(
        &["users; DROP TABLE users"],
        &[("users; DROP TABLE users", &["id"])],
    );
    let validator = SchemaValidator::new(&db, SchemaValidationMode::Strict);

    let err = validator
        .validate_and_apply(
            "svc",
            std::path::Path::new("/tmp"),
            &[schema_def("users; DROP TABLE users", "users.sql", &["id"])],
        )
        .await
        .expect_err("hostile identifier must be rejected");

    assert!(err.to_string().contains("invalid SQL identifier"));
}
