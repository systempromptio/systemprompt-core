//! The JSON handlers of `/api/v1/agent-runs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_agent_runs::execute::outputs;
use systemprompt_agent_runs::repository::{AgentRunRow, Enqueued, NewAgentRun};
use systemprompt_agent_runs::{
    AgentRunsError, McpServerSpec, RunEvent, RunLayout, RunRequest, origin,
};
use systemprompt_identifiers::{AgentRunId, HostSessionId, SessionId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

use super::AgentRunsState;
use crate::error::ApiHttpError;

pub(super) const EVENT_PAGE: i64 = 500;

#[derive(Debug, Deserialize)]
pub(super) struct EnqueueBody {
    origin_kind: String,
    idempotency_key: String,
    request: RunRequest,
}

#[derive(Debug, Serialize)]
struct Queued {
    id: AgentRunId,
    existing: bool,
}

#[derive(Debug, Serialize)]
struct RunView {
    id: AgentRunId,
    origin_kind: String,
    origin_id: String,
    host: String,
    status: String,
    error_class: Option<String>,
    error: Option<String>,
    harness_digest: Option<String>,
    gateway_session_id: Option<SessionId>,
    host_session_id: Option<HostSessionId>,
    turns: Option<i32>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cost_microdollars: Option<i64>,
    // JSON: the host's structured answer, shaped by the request's output schema.
    structured_output: Option<serde_json::Value>,
    // JSON: the workspace diff summary the executor recorded.
    diff_summary: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

impl From<AgentRunRow> for RunView {
    fn from(row: AgentRunRow) -> Self {
        Self {
            id: row.id,
            origin_kind: row.origin_kind,
            origin_id: row.origin_id,
            host: row.host,
            status: row.status,
            error_class: row.error_class,
            error: row.error,
            harness_digest: row.harness_digest,
            gateway_session_id: row.gateway_session_id,
            host_session_id: row.host_session_id,
            turns: row.turns,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cost_microdollars: row.cost_microdollars,
            structured_output: row.structured_output,
            diff_summary: row.diff_summary,
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct EventsQuery {
    after: Option<i64>,
}

#[derive(Debug, Serialize)]
struct EventsPage {
    events: Vec<RunEvent>,
}

#[derive(Debug, Serialize)]
struct OutputList {
    files: Vec<outputs::OutputFile>,
}

pub(super) fn internal(error: AgentRunsError) -> ApiHttpError {
    ApiHttpError::from(ApiError::internal("Agent run storage failed", error))
}

pub(super) async fn owned_run(
    state: &AgentRunsState,
    req_ctx: &RequestContext,
    id: &str,
) -> Result<AgentRunRow, ApiHttpError> {
    let id = AgentRunId::try_new(id).map_err(ApiError::from)?;
    match state.runs.find_run(&id).await.map_err(internal)? {
        Some(row) if &row.user_id == req_ctx.user_id() => Ok(row),
        _ => Err(ApiHttpError::not_found(format!(
            "agent run '{id}' not found"
        ))),
    }
}

fn refusal(body: &mut EnqueueBody, layout: &RunLayout) -> Option<String> {
    if !origin::is_registered(&body.origin_kind) {
        return Some(format!(
            "origin_kind '{}' is not registered; registered: {:?}",
            body.origin_kind,
            origin::registered()
        ));
    }
    let key = body.idempotency_key.trim();
    if key.is_empty() || key.len() > 256 {
        return Some("idempotency_key must be 1-256 characters".to_owned());
    }
    let request = &mut body.request;
    if !request.add_dirs.is_empty() || !request.plugin_dirs.is_empty() {
        return Some(
            "add_dirs and plugin_dirs name server paths and are not accepted over the API"
                .to_owned(),
        );
    }
    if request
        .mcp_servers
        .values()
        .any(|spec| matches!(spec, McpServerSpec::Stdio { .. }))
    {
        return Some(
            "stdio MCP servers run server commands and are not accepted over the API".to_owned(),
        );
    }
    request.workspace.clone_from(&layout.workspace);
    None
}

pub(super) async fn enqueue(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AgentRunsState>,
    Json(mut body): Json<EnqueueBody>,
) -> Result<Response, ApiHttpError> {
    let id = AgentRunId::generate();
    if let Some(reason) = refusal(&mut body, &state.layouts.layout(&id)) {
        return Err(ApiHttpError::bad_request(reason));
    }
    let key = body.idempotency_key.trim().to_owned();
    let enqueued = state
        .runs
        .enqueue_idempotent(
            NewAgentRun {
                id: &id,
                origin_kind: &body.origin_kind,
                origin_id: &key,
                user_id: req_ctx.user_id(),
                request: &body.request,
                not_before: None,
                task_id: None,
                context_id: None,
            },
            &key,
        )
        .await
        .map_err(internal)?;
    let (id, existing) = match enqueued {
        Enqueued::Created(id) => (id, false),
        Enqueued::Existing(id) => (id, true),
        Enqueued::Conflict(_) => {
            return Err(ApiHttpError::conflict(
                "idempotency_key is already bound to a different request",
            ));
        },
    };
    let location = format!(
        "{}/{id}",
        systemprompt_models::modules::ApiPaths::AGENT_RUNS
    );
    let status = if existing {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    Ok((
        status,
        [(header::LOCATION, location)],
        Json(Queued { id, existing }),
    )
        .into_response())
}

pub(super) async fn get_run(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AgentRunsState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let row = owned_run(&state, &req_ctx, &id).await?;
    Ok(Json(RunView::from(row)))
}

pub(super) async fn cancel(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AgentRunsState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let row = owned_run(&state, &req_ctx, &id).await?;
    if !state.runs.cancel_queued(&row.id).await.map_err(internal)? {
        state
            .runs
            .update_cancel_requested(&row.id)
            .await
            .map_err(internal)?;
    }
    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn list_events(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AgentRunsState>,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let row = owned_run(&state, &req_ctx, &id).await?;
    let events = state
        .runs
        .list_events_after(&row.id, query.after.unwrap_or(-1), EVENT_PAGE)
        .await
        .map_err(internal)?;
    Ok(Json(EventsPage { events }))
}

fn run_layout(row: &AgentRunRow) -> Option<RunLayout> {
    row.run_dir.as_deref().map(RunLayout::new)
}

pub(super) async fn list_outputs(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AgentRunsState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let row = owned_run(&state, &req_ctx, &id).await?;
    let files = match run_layout(&row) {
        Some(layout) => outputs::list(&layout).await.map_err(internal)?,
        None => Vec::new(),
    };
    Ok(Json(OutputList { files }))
}

pub(super) async fn read_output(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AgentRunsState>,
    Path((id, path)): Path<(String, String)>,
) -> Result<Response, ApiHttpError> {
    let row = owned_run(&state, &req_ctx, &id).await?;
    let missing = || ApiHttpError::not_found(format!("output '{path}' not found"));
    let layout = run_layout(&row).ok_or_else(missing)?;
    let bytes = outputs::read(&layout, &path)
        .await
        .map_err(internal)?
        .ok_or_else(missing)?;
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream")],
        Body::from(bytes),
    )
        .into_response())
}
