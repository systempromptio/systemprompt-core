use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_database::{
    DatabaseInfo, DatabaseProvider, DatabaseResult, DatabaseTransaction, JsonRow, QueryResult,
    QuerySelector, RepositoryError, ToDbValue, replica_status, validate_column_exists,
    validate_table_exists,
};

#[derive(Debug)]
struct ResultProvider {
    result: QueryResult,
    pool: Arc<sqlx::PgPool>,
}

impl ResultProvider {
    fn new(rows: Vec<HashMap<String, serde_json::Value>>) -> Self {
        Self {
            result: QueryResult {
                row_count: rows.len(),
                rows,
                columns: Vec::new(),
                execution_time_ms: 0,
            },
            pool: Arc::new(
                sqlx::postgres::PgPoolOptions::new()
                    .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
                    .expect("lazy fixture pool"),
            ),
        }
    }
}

#[async_trait]
impl DatabaseProvider for ResultProvider {
    fn get_postgres_pool(&self) -> Arc<sqlx::PgPool> {
        Arc::clone(&self.pool)
    }
    async fn execute(&self, _: &dyn QuerySelector, _: &[&dyn ToDbValue]) -> DatabaseResult<u64> {
        Ok(0)
    }
    async fn execute_raw(&self, _: &str) -> DatabaseResult<()> {
        Ok(())
    }
    async fn fetch_all(
        &self,
        _: &dyn QuerySelector,
        _: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>> {
        Ok(Vec::new())
    }
    async fn fetch_one(
        &self,
        _: &dyn QuerySelector,
        _: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow> {
        Ok(JsonRow::new())
    }
    async fn fetch_optional(
        &self,
        _: &dyn QuerySelector,
        _: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>> {
        Ok(None)
    }
    async fn begin_transaction(&self) -> DatabaseResult<Box<dyn DatabaseTransaction>> {
        Err(RepositoryError::internal("unused fixture transaction"))
    }
    async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
        Err(RepositoryError::internal("unused fixture info"))
    }
    async fn test_connection(&self) -> DatabaseResult<()> {
        Ok(())
    }
    async fn execute_batch(&self, _: &str) -> DatabaseResult<()> {
        Ok(())
    }
    async fn query_raw(&self, _: &dyn QuerySelector) -> DatabaseResult<QueryResult> {
        Ok(self.result.clone())
    }
    async fn query_raw_with(
        &self,
        _: &dyn QuerySelector,
        _: &[&dyn ToDbValue],
    ) -> DatabaseResult<QueryResult> {
        Ok(self.result.clone())
    }
}

fn row(entries: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

#[tokio::test]
async fn replica_probe_rejects_missing_or_untyped_recovery_state() {
    for (provider, expected) in [
        (ResultProvider::new(Vec::new()), "returned no row"),
        (
            ResultProvider::new(vec![row(&[("lag_secs", json!(2.5))])]),
            "lacks in_recovery",
        ),
        (
            ResultProvider::new(vec![row(&[("in_recovery", json!("false"))])]),
            "lacks in_recovery",
        ),
    ] {
        let error = replica_status(&provider)
            .await
            .expect_err("malformed probe rejected");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[tokio::test]
async fn replica_probe_preserves_primary_and_standby_lag_semantics() {
    let primary = ResultProvider::new(vec![row(&[
        ("in_recovery", json!(false)),
        ("lag_secs", serde_json::Value::Null),
    ])]);
    let status = replica_status(&primary).await.expect("primary status");
    assert!(!status.in_recovery);
    assert_eq!(status.replay_lag_secs, None);

    let standby = ResultProvider::new(vec![row(&[
        ("in_recovery", json!(true)),
        ("lag_secs", json!(3.25)),
    ])]);
    let status = replica_status(&standby).await.expect("standby status");
    assert!(status.in_recovery);
    assert_eq!(status.replay_lag_secs, Some(3.25));
}

#[tokio::test]
async fn table_and_column_probes_reject_absent_or_non_boolean_results() {
    for provider in [
        ResultProvider::new(Vec::new()),
        ResultProvider::new(vec![row(&[("exists", json!("yes"))])]),
        ResultProvider::new(vec![row(&[("other", json!(true))])]),
    ] {
        let table_error = validate_table_exists(&provider, "managed_resources")
            .await
            .expect_err("malformed table probe rejected");
        assert!(
            table_error.to_string().contains("managed_resources"),
            "{table_error}"
        );
        let column_error = validate_column_exists(&provider, "managed_resources", "owner_id")
            .await
            .expect_err("malformed column probe rejected");
        assert!(
            column_error
                .to_string()
                .contains("managed_resources.owner_id"),
            "{column_error}"
        );
    }
}
