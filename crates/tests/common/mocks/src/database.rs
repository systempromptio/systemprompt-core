use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use systemprompt_database::{
    DatabaseInfo, DatabaseProvider, DatabaseResult, DatabaseTransaction, DbValue, JsonRow,
    QueryResult, QuerySelector, RepositoryError, ToDbValue,
};

#[derive(Debug, Clone)]
pub enum MockDbResponse {
    FetchAll(Result<Vec<JsonRow>, String>),
    FetchOne(Result<JsonRow, String>),
    FetchOptional(Result<Option<JsonRow>, String>),
    FetchScalar(Result<DbValue, String>),
    Execute(Result<u64, String>),
    ExecuteRaw(Result<(), String>),
    ExecuteBatch(Result<(), String>),
    QueryRaw(Result<QueryResult, String>),
    TestConnection(Result<(), String>),
    DatabaseInfo(Result<DatabaseInfo, String>),
    BeginTransaction(Result<(), String>),
}

#[derive(Debug)]
pub struct MockDatabaseProvider {
    responses: Arc<Mutex<VecDeque<MockDbResponse>>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl MockDatabaseProvider {
    pub fn builder() -> MockDatabaseProviderBuilder {
        MockDatabaseProviderBuilder {
            responses: VecDeque::new(),
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("lock poisoned").clone()
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().expect("lock poisoned").len()
    }

    fn record_call(&self, method: &str) {
        self.calls
            .lock()
            .expect("lock poisoned")
            .push(method.to_string());
    }

    fn next_response(&self) -> Option<MockDbResponse> {
        self.responses.lock().expect("lock poisoned").pop_front()
    }
}

impl Default for MockDatabaseProvider {
    fn default() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

pub struct MockDatabaseProviderBuilder {
    responses: VecDeque<MockDbResponse>,
}

impl MockDatabaseProviderBuilder {
    pub fn with_fetch_all_result(mut self, result: Result<Vec<JsonRow>, String>) -> Self {
        self.responses.push_back(MockDbResponse::FetchAll(result));
        self
    }

    pub fn with_fetch_one_result(mut self, result: Result<JsonRow, String>) -> Self {
        self.responses.push_back(MockDbResponse::FetchOne(result));
        self
    }

    pub fn with_fetch_optional_result(mut self, result: Result<Option<JsonRow>, String>) -> Self {
        self.responses
            .push_back(MockDbResponse::FetchOptional(result));
        self
    }

    pub fn with_fetch_scalar_result(mut self, result: Result<DbValue, String>) -> Self {
        self.responses
            .push_back(MockDbResponse::FetchScalar(result));
        self
    }

    pub fn with_execute_result(mut self, result: Result<u64, String>) -> Self {
        self.responses.push_back(MockDbResponse::Execute(result));
        self
    }

    pub fn with_execute_raw_result(mut self, result: Result<(), String>) -> Self {
        self.responses.push_back(MockDbResponse::ExecuteRaw(result));
        self
    }

    pub fn with_query_raw_result(mut self, result: Result<QueryResult, String>) -> Self {
        self.responses.push_back(MockDbResponse::QueryRaw(result));
        self
    }

    pub fn with_database_info_result(mut self, result: Result<DatabaseInfo, String>) -> Self {
        self.responses
            .push_back(MockDbResponse::DatabaseInfo(result));
        self
    }

    pub fn with_error(mut self, error_message: &str) -> Self {
        self.responses
            .push_back(MockDbResponse::FetchAll(Err(error_message.to_string())));
        self
    }

    pub fn with_response(mut self, response: MockDbResponse) -> Self {
        self.responses.push_back(response);
        self
    }

    pub fn build(self) -> MockDatabaseProvider {
        MockDatabaseProvider {
            responses: Arc::new(Mutex::new(self.responses)),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

fn convert_result<T>(result: std::result::Result<T, String>) -> DatabaseResult<T> {
    result.map_err(RepositoryError::internal)
}

#[async_trait]
impl DatabaseProvider for MockDatabaseProvider {
    fn get_postgres_pool(&self) -> Option<Arc<sqlx::PgPool>> {
        None
    }

    fn is_postgres(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        self.record_call("execute");
        match self.next_response() {
            Some(MockDbResponse::Execute(result)) => convert_result(result),
            Some(_) => Ok(0),
            None => Ok(0),
        }
    }

    async fn execute_raw(&self, _sql: &str) -> DatabaseResult<()> {
        self.record_call("execute_raw");
        match self.next_response() {
            Some(MockDbResponse::ExecuteRaw(result)) => convert_result(result),
            Some(_) => Ok(()),
            None => Ok(()),
        }
    }

    async fn fetch_all(
        &self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>> {
        self.record_call("fetch_all");
        match self.next_response() {
            Some(MockDbResponse::FetchAll(result)) => convert_result(result),
            Some(_) => Ok(vec![]),
            None => Ok(vec![]),
        }
    }

    async fn fetch_one(
        &self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow> {
        self.record_call("fetch_one");
        match self.next_response() {
            Some(MockDbResponse::FetchOne(result)) => convert_result(result),
            Some(_) => Ok(JsonRow::new()),
            None => Ok(JsonRow::new()),
        }
    }

    async fn fetch_optional(
        &self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>> {
        self.record_call("fetch_optional");
        match self.next_response() {
            Some(MockDbResponse::FetchOptional(result)) => convert_result(result),
            Some(_) => Ok(None),
            None => Ok(None),
        }
    }

    async fn fetch_scalar_value(
        &self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<DbValue> {
        self.record_call("fetch_scalar_value");
        match self.next_response() {
            Some(MockDbResponse::FetchScalar(result)) => convert_result(result),
            Some(_) => Ok(DbValue::NullString),
            None => Ok(DbValue::NullString),
        }
    }

    async fn begin_transaction(&self) -> DatabaseResult<Box<dyn DatabaseTransaction>> {
        self.record_call("begin_transaction");
        Ok(Box::new(MockDatabaseTransaction::default()))
    }

    async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
        self.record_call("get_database_info");
        match self.next_response() {
            Some(MockDbResponse::DatabaseInfo(result)) => convert_result(result),
            Some(_) => Ok(DatabaseInfo {
                path: String::new(),
                size: 0,
                version: "mock".to_string(),
                tables: vec![],
            }),
            None => Ok(DatabaseInfo {
                path: String::new(),
                size: 0,
                version: "mock".to_string(),
                tables: vec![],
            }),
        }
    }

    async fn test_connection(&self) -> DatabaseResult<()> {
        self.record_call("test_connection");
        match self.next_response() {
            Some(MockDbResponse::TestConnection(result)) => convert_result(result),
            Some(_) => Ok(()),
            None => Ok(()),
        }
    }

    async fn execute_batch(&self, _sql: &str) -> DatabaseResult<()> {
        self.record_call("execute_batch");
        match self.next_response() {
            Some(MockDbResponse::ExecuteBatch(result)) => convert_result(result),
            Some(_) => Ok(()),
            None => Ok(()),
        }
    }

    async fn query_raw(&self, _query: &dyn QuerySelector) -> DatabaseResult<QueryResult> {
        self.record_call("query_raw");
        match self.next_response() {
            Some(MockDbResponse::QueryRaw(result)) => convert_result(result),
            Some(_) => Ok(QueryResult::default()),
            None => Ok(QueryResult::default()),
        }
    }

    async fn query_raw_with(
        &self,
        _query: &dyn QuerySelector,
        _params: Vec<serde_json::Value>,
    ) -> DatabaseResult<QueryResult> {
        self.record_call("query_raw_with");
        match self.next_response() {
            Some(MockDbResponse::QueryRaw(result)) => convert_result(result),
            Some(_) => Ok(QueryResult::default()),
            None => Ok(QueryResult::default()),
        }
    }
}

#[derive(Debug, Default)]
struct MockDatabaseTransaction {
    responses: VecDeque<MockDbResponse>,
    calls: Vec<String>,
}

#[async_trait]
impl DatabaseTransaction for MockDatabaseTransaction {
    async fn execute(
        &mut self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        self.calls.push("execute".to_string());
        match self.responses.pop_front() {
            Some(MockDbResponse::Execute(result)) => convert_result(result),
            Some(_) => Ok(0),
            None => Ok(0),
        }
    }

    async fn fetch_all(
        &mut self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>> {
        self.calls.push("fetch_all".to_string());
        match self.responses.pop_front() {
            Some(MockDbResponse::FetchAll(result)) => convert_result(result),
            Some(_) => Ok(vec![]),
            None => Ok(vec![]),
        }
    }

    async fn fetch_one(
        &mut self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow> {
        self.calls.push("fetch_one".to_string());
        match self.responses.pop_front() {
            Some(MockDbResponse::FetchOne(result)) => convert_result(result),
            Some(_) => Ok(JsonRow::new()),
            None => Ok(JsonRow::new()),
        }
    }

    async fn fetch_optional(
        &mut self,
        _query: &dyn QuerySelector,
        _params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>> {
        self.calls.push("fetch_optional".to_string());
        match self.responses.pop_front() {
            Some(MockDbResponse::FetchOptional(result)) => convert_result(result),
            Some(_) => Ok(None),
            None => Ok(None),
        }
    }

    async fn commit(self: Box<Self>) -> DatabaseResult<()> {
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> DatabaseResult<()> {
        Ok(())
    }
}
