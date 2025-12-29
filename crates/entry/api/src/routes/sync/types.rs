use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ExportQuery {
    pub tables: Option<String>,
}

#[derive(Serialize)]
pub struct DatabaseExport {
    pub services: Vec<serde_json::Value>,
    pub skills: Vec<serde_json::Value>,
    pub contexts: Vec<serde_json::Value>,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub record_counts: RecordCounts,
}

#[derive(Serialize)]
pub struct RecordCounts {
    pub services: usize,
    pub skills: usize,
    pub contexts: usize,
}

#[derive(Deserialize)]
pub struct DatabaseImportRequest {
    #[serde(default)]
    pub services: Vec<serde_json::Value>,
    #[serde(default)]
    pub skills: Vec<serde_json::Value>,
    #[serde(default)]
    pub contexts: Vec<serde_json::Value>,
    pub merge_strategy: Option<String>,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub imported_at: chrono::DateTime<chrono::Utc>,
    pub results: ImportResults,
}

#[derive(Serialize)]
pub struct ImportResults {
    pub services: TableResult,
    pub skills: TableResult,
    pub contexts: TableResult,
}

#[derive(Serialize, Default, Clone)]
pub struct TableResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub deleted: usize,
}

#[derive(Serialize)]
pub struct ExportError {
    pub error: String,
}

pub type ApiResult<T> = Result<T, (StatusCode, Json<ExportError>)>;

pub fn to_api_error(e: impl std::fmt::Display) -> (StatusCode, Json<ExportError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ExportError {
            error: e.to_string(),
        }),
    )
}
