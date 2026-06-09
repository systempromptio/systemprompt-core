//! Pure-path tests for [`SchemaValidator`] that do not require a live database.
//!
//! [`SchemaValidationMode::Skip`] returns without touching the provider, so the
//! `validate_and_apply` call is exercisable with a synthetic config alone.

use systemprompt_mcp::services::schema::{
    SchemaValidationMode, SchemaValidationReport, SchemaValidator,
};
use systemprompt_models::mcp::deployment::SchemaDefinition;

struct NoopProvider;

impl std::fmt::Debug for NoopProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NoopProvider")
    }
}

#[async_trait::async_trait]
impl systemprompt_database::DatabaseProvider for NoopProvider {
    async fn execute(
        &self,
        _q: &dyn systemprompt_database::QuerySelector,
        _p: &[&dyn systemprompt_database::ToDbValue],
    ) -> systemprompt_database::DatabaseResult<u64> {
        Ok(0)
    }

    async fn execute_raw(&self, _sql: &str) -> systemprompt_database::DatabaseResult<()> {
        Ok(())
    }

    async fn fetch_all(
        &self,
        _q: &dyn systemprompt_database::QuerySelector,
        _p: &[&dyn systemprompt_database::ToDbValue],
    ) -> systemprompt_database::DatabaseResult<Vec<systemprompt_database::JsonRow>> {
        Ok(vec![])
    }

    async fn fetch_one(
        &self,
        _q: &dyn systemprompt_database::QuerySelector,
        _p: &[&dyn systemprompt_database::ToDbValue],
    ) -> systemprompt_database::DatabaseResult<systemprompt_database::JsonRow> {
        Err(systemprompt_database::RepositoryError::not_found("no row"))
    }

    async fn fetch_optional(
        &self,
        _q: &dyn systemprompt_database::QuerySelector,
        _p: &[&dyn systemprompt_database::ToDbValue],
    ) -> systemprompt_database::DatabaseResult<Option<systemprompt_database::JsonRow>> {
        Ok(None)
    }

    async fn fetch_scalar_value(
        &self,
        _q: &dyn systemprompt_database::QuerySelector,
        _p: &[&dyn systemprompt_database::ToDbValue],
    ) -> systemprompt_database::DatabaseResult<systemprompt_database::DbValue> {
        Ok(systemprompt_database::DbValue::NullString)
    }

    async fn begin_transaction(
        &self,
    ) -> systemprompt_database::DatabaseResult<Box<dyn systemprompt_database::DatabaseTransaction>>
    {
        Err(systemprompt_database::RepositoryError::not_found("no tx"))
    }

    async fn get_database_info(
        &self,
    ) -> systemprompt_database::DatabaseResult<systemprompt_database::DatabaseInfo> {
        Err(systemprompt_database::RepositoryError::not_found("no info"))
    }

    async fn test_connection(&self) -> systemprompt_database::DatabaseResult<()> {
        Ok(())
    }

    async fn execute_batch(&self, _sql: &str) -> systemprompt_database::DatabaseResult<()> {
        Ok(())
    }

    async fn query_raw(
        &self,
        _q: &dyn systemprompt_database::QuerySelector,
    ) -> systemprompt_database::DatabaseResult<systemprompt_database::QueryResult> {
        Ok(systemprompt_database::QueryResult::default())
    }

    async fn query_raw_with(
        &self,
        _q: &dyn systemprompt_database::QuerySelector,
        _p: &[&dyn systemprompt_database::ToDbValue],
    ) -> systemprompt_database::DatabaseResult<systemprompt_database::QueryResult> {
        Ok(systemprompt_database::QueryResult::default())
    }
}

#[tokio::test]
async fn skip_mode_returns_ok_with_warning() {
    let db = NoopProvider;
    let validator = SchemaValidator::new(&db, SchemaValidationMode::Skip);
    let schemas = vec![SchemaDefinition {
        table: "my_table".to_owned(),
        file: "001_my_table.sql".to_owned(),
        required_columns: vec!["id".to_owned()],
    }];

    let report = validator
        .validate_and_apply("test-service", std::path::Path::new("/tmp"), &schemas)
        .await
        .expect("skip mode must not error");

    assert_eq!(report.validated, 0);
    assert_eq!(report.created, 0);
    assert!(report.errors.is_empty());
    assert!(
        !report.warnings.is_empty(),
        "skip mode should add a warning"
    );
    assert!(
        report.warnings[0].contains("skipped")
            || report.warnings[0].to_lowercase().contains("skip")
    );
}

#[tokio::test]
async fn skip_mode_empty_schemas_returns_ok() {
    let db = NoopProvider;
    let validator = SchemaValidator::new(&db, SchemaValidationMode::Skip);

    let report = validator
        .validate_and_apply("svc", std::path::Path::new("/tmp"), &[])
        .await
        .expect("empty schemas with skip should succeed");

    assert_eq!(report.validated, 0);
    assert_eq!(report.created, 0);
}

#[test]
fn schema_validation_mode_from_string_coverage() {
    assert_eq!(
        SchemaValidationMode::from_string("strict"),
        SchemaValidationMode::Strict
    );
    assert_eq!(
        SchemaValidationMode::from_string("skip"),
        SchemaValidationMode::Skip
    );
    assert_eq!(
        SchemaValidationMode::from_string("auto_migrate"),
        SchemaValidationMode::AutoMigrate
    );
    assert_eq!(
        SchemaValidationMode::from_string("STRICT"),
        SchemaValidationMode::Strict
    );
    assert_eq!(
        SchemaValidationMode::from_string("SKIP"),
        SchemaValidationMode::Skip
    );
    assert_eq!(
        SchemaValidationMode::from_string("unknown"),
        SchemaValidationMode::AutoMigrate
    );
}

#[test]
fn schema_validation_mode_eq_and_copy() {
    let a = SchemaValidationMode::Strict;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, SchemaValidationMode::Skip);
    assert_ne!(a, SchemaValidationMode::AutoMigrate);
}

#[test]
fn schema_validation_mode_debug() {
    let modes = [
        SchemaValidationMode::AutoMigrate,
        SchemaValidationMode::Strict,
        SchemaValidationMode::Skip,
    ];
    for m in modes {
        let s = format!("{m:?}");
        assert!(!s.is_empty());
    }
}

#[test]
fn schema_validation_report_new_fields() {
    let r = SchemaValidationReport::new("my-svc".to_owned());
    assert_eq!(r.service_name, "my-svc");
    assert_eq!(r.validated, 0);
    assert_eq!(r.created, 0);
    assert!(r.errors.is_empty());
    assert!(r.warnings.is_empty());
}

#[test]
fn schema_validation_report_merge_counters() {
    let mut a = SchemaValidationReport::new("a".to_owned());
    a.validated = 2;
    a.created = 1;
    a.errors.push("e1".to_owned());
    a.warnings.push("w1".to_owned());

    let mut b = SchemaValidationReport::new("b".to_owned());
    b.validated = 3;
    b.created = 2;
    b.errors.push("e2".to_owned());
    b.errors.push("e3".to_owned());
    b.warnings.push("w2".to_owned());

    a.merge(b);

    assert_eq!(a.validated, 5);
    assert_eq!(a.created, 3);
    assert_eq!(a.errors.len(), 3);
    assert_eq!(a.warnings.len(), 2);
    assert_eq!(a.service_name, "a");
}

#[test]
fn schema_validation_report_serialize_round_trip() {
    let mut r = SchemaValidationReport::new("svc-rt".to_owned());
    r.validated = 7;
    r.created = 3;
    r.errors.push("an error".to_owned());

    let json = serde_json::to_string(&r).expect("serialize");
    let back: SchemaValidationReport = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.service_name, "svc-rt");
    assert_eq!(back.validated, 7);
    assert_eq!(back.created, 3);
    assert_eq!(back.errors, vec!["an error"]);
    assert!(back.warnings.is_empty());
}

#[test]
fn schema_validation_report_clone() {
    let mut r = SchemaValidationReport::new("clone-svc".to_owned());
    r.validated = 1;
    r.errors.push("err".to_owned());

    let c = r.clone();
    assert_eq!(c.service_name, r.service_name);
    assert_eq!(c.validated, r.validated);
    assert_eq!(c.errors, r.errors);
}
