use rmcp::{
    model::{CallToolRequestParam, CallToolResult, Content},
    service::RequestContext,
    ErrorData as McpError, RoleServer,
};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::identifiers::{ArtifactId, McpExecutionId};
use systemprompt::models::artifacts::{
    Column, ColumnType, ExecutionMetadata, TableArtifact, TableHints, ToolResponse,
};
use systemprompt::scheduler::models::ScheduledJob;
use systemprompt::scheduler::repository::SchedulerRepository;
use systemprompt::system::AppContext;

#[must_use]
pub fn jobs_input_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "execute_job": {
                "type": "string",
                "description": "Optional job name to execute (leave blank to just list jobs)",
                "enum": [
                    "cleanup_anonymous_users",
                    "cleanup_inactive_sessions",
                    "database_cleanup",
                    "publish_content",
                    "evaluate_conversations",
                    "ingest_content",
                    "ingest_files",
                    "optimize_images",
                    "regenerate_static_content",
                    "rebuild_static_site"
                ]
            }
        }
    })
}

#[must_use]
pub fn jobs_output_schema() -> JsonValue {
    json!({
        "type": "object",
        "description": "Table of all scheduler jobs with current status",
        "properties": {
            "x-artifact-type": {"type": "string", "enum": ["table"]},
            "columns": {"type": "array"},
            "rows": {"type": "array"}
        },
        "required": ["x-artifact-type", "columns", "rows"]
    })
}

pub async fn handle_jobs(
    pool: &DbPool,
    request: CallToolRequestParam,
    _ctx: RequestContext<RoleServer>,
    _app_context: Arc<AppContext>,
    mcp_execution_id: &McpExecutionId,
) -> Result<CallToolResult, McpError> {
    let execute_job = request
        .arguments
        .as_ref()
        .and_then(|args| args.get("execute_job"))
        .and_then(|v| v.as_str());

    if let Some(job_name) = execute_job {
        tracing::warn!(job_name = %job_name, "Job execution not supported in this version");
        return Err(McpError::invalid_request(
            format!("Job execution is not supported in this version. Job: {job_name}"),
            None,
        ));
    }

    let repo = SchedulerRepository::new(pool)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    let jobs = repo
        .list_enabled_jobs()
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let table = build_jobs_table(jobs);

    let metadata = ExecutionMetadata::new().tool("jobs");
    let artifact_id = ArtifactId::new(uuid::Uuid::new_v4().to_string());
    let tool_response = ToolResponse::new(
        artifact_id,
        mcp_execution_id.clone(),
        table,
        metadata.clone(),
    );

    Ok(CallToolResult {
        content: vec![Content::text(format!(
            "Scheduler Jobs{}",
            execute_job
                .map(|j| format!(" (executing: {j})"))
                .unwrap_or_default()
        ))],
        structured_content: Some(tool_response.to_json()),
        is_error: Some(false),
        meta: metadata.to_meta(),
    })
}

fn build_jobs_table(jobs: Vec<ScheduledJob>) -> TableArtifact {
    let columns = vec![
        Column::new("job_name", ColumnType::String).with_header("Job Name"),
        Column::new("schedule", ColumnType::String).with_header("Schedule"),
        Column::new("enabled", ColumnType::Boolean).with_header("Enabled"),
        Column::new("last_run", ColumnType::String).with_header("Last Run"),
        Column::new("last_status", ColumnType::String).with_header("Status"),
        Column::new("run_count", ColumnType::Number).with_header("Run Count"),
        Column::new("last_error", ColumnType::String).with_header("Error"),
    ];

    let rows: Vec<serde_json::Value> = jobs
        .iter()
        .map(|job| {
            json!({
                "job_name": job.job_name,
                "schedule": job.schedule,
                "enabled": job.enabled,
                "last_run": job.last_run.map_or_else(|| "Never".to_string(), |dt| dt.to_rfc3339()),
                "last_status": job.last_status.as_deref().unwrap_or("—"),
                "run_count": job.run_count,
                "last_error": job.last_error.as_deref().unwrap_or(""),
            })
        })
        .collect();

    TableArtifact::new(columns).with_rows(rows).with_hints(
        TableHints::new()
            .with_sortable(vec![
                "job_name".to_string(),
                "last_run".to_string(),
                "run_count".to_string(),
            ])
            .filterable(),
    )
}
