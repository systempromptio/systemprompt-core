//! Evaluator worker transport with server-owned identity and fenced mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::EvaluationWorkerState;
use super::error::WorkerHttpError;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use serde::Deserialize;
use systemprompt_evaluation::experiments::execution::{EvidenceArchive, ExecutionEvidence};
use systemprompt_evaluation::experiments::records::ExecutionRecord;
use systemprompt_evaluation::repository::experiments::{
    DeterministicMeasurement, ExecutionCompletion, ExecutionLease, TerminalOutcome, WorkerRecord,
};

async fn authenticate(
    state: &EvaluationWorkerState,
    headers: &HeaderMap,
) -> Result<WorkerRecord, WorkerHttpError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(WorkerHttpError::Unauthorized)?;
    match state.workers.authenticate(token, &state.environment).await {
        Ok(worker) => Ok(worker),
        Err(systemprompt_evaluation::EvaluationError::ResourceNotFound(_)) => {
            Err(WorkerHttpError::Unauthorized)
        },
        Err(error) => Err(error.into()),
    }
}

fn verify_worker(worker: &WorkerRecord, lease: &ExecutionLease) -> Result<(), WorkerHttpError> {
    if worker.id != lease.worker_id {
        return Err(WorkerHttpError::Unauthorized);
    }
    Ok(())
}

pub(super) async fn claim(
    State(state): State<EvaluationWorkerState>,
    headers: HeaderMap,
) -> Result<Json<Option<ExecutionRecord>>, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    Ok(Json(
        state
            .experiments
            .claim(&worker.owner_id, &worker.id)
            .await?,
    ))
}

pub(super) async fn heartbeat(
    State(state): State<EvaluationWorkerState>,
    headers: HeaderMap,
    Json(lease): Json<ExecutionLease>,
) -> Result<StatusCode, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    verify_worker(&worker, &lease)?;
    state
        .experiments
        .heartbeat(&worker.owner_id, &lease)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidenceSubmission {
    lease: ExecutionLease,
    evidence: ExecutionEvidence,
    artifacts: EvidenceArchive,
}

pub(super) async fn evidence(
    State(state): State<EvaluationWorkerState>,
    headers: HeaderMap,
    Json(input): Json<EvidenceSubmission>,
) -> Result<StatusCode, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    verify_worker(&worker, &input.lease)?;
    state
        .evidence
        .submit(
            &worker.owner_id,
            &input.lease,
            &input.evidence,
            &input.artifacts,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionSubmission {
    lease: ExecutionLease,
    completion: ExecutionCompletion,
}

pub(super) async fn complete(
    State(state): State<EvaluationWorkerState>,
    headers: HeaderMap,
    Json(input): Json<CompletionSubmission>,
) -> Result<StatusCode, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    verify_worker(&worker, &input.lease)?;
    if input.completion.outcome == TerminalOutcome::Completed {
        let evidence = state
            .evidence
            .get(&worker.owner_id, &input.lease.execution_id)
            .await?;
        if evidence.fencing_token != input.lease.fencing_token
            || !evidence.cleanup_confirmed
            || evidence.exit_code != Some(0)
        {
            return Err(
                systemprompt_evaluation::EvaluationError::ExperimentConflict(
                    "Successful completion requires clean, fenced execution evidence".to_owned(),
                )
                .into(),
            );
        }
    }
    state
        .experiments
        .complete(&worker.owner_id, &input.lease, &input.completion)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn access(
    State(state): State<EvaluationWorkerState>,
    headers: HeaderMap,
    Json(lease): Json<ExecutionLease>,
) -> Result<impl axum::response::IntoResponse, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    verify_worker(&worker, &lease)?;
    let access = state.capabilities.issue(&worker.owner_id, &lease).await?;
    Ok(([("cache-control", "no-store")], Json(access)))
}


pub(super) async fn assignment(
    State(state): State<EvaluationWorkerState>,
    headers: HeaderMap,
    Json(lease): Json<ExecutionLease>,
) -> Result<impl axum::response::IntoResponse, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    verify_worker(&worker, &lease)?;
    let assignment = state.assignments.get(&worker, &lease).await?;
    Ok(([("cache-control", "no-store")], Json(assignment)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EventSubmission {
    lease: ExecutionLease,
    event: systemprompt_evaluation::repository::experiments::ExecutionEvent,
}

pub(super) async fn event(
    State(state): State<EvaluationWorkerState>,
    headers: HeaderMap,
    Json(input): Json<EventSubmission>,
) -> Result<StatusCode, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    verify_worker(&worker, &input.lease)?;
    state
        .events
        .append(&worker, &input.lease, &input.event)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ApprovalSubmission {
    lease: ExecutionLease,
    operation: serde_json::Value,
    precondition_digest: String,
}

pub(super) async fn request_approval(
    State(state): State<EvaluationWorkerState>, headers: HeaderMap, Json(input): Json<ApprovalSubmission>,
) -> Result<impl axum::response::IntoResponse, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?; verify_worker(&worker, &input.lease)?;
    Ok((StatusCode::ACCEPTED, Json(state.lifecycle.request_approval(&worker.owner_id, &input.lease, input.operation, &input.precondition_digest).await?)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasurementSubmission { lease: ExecutionLease, measurement: DeterministicMeasurement }

pub(super) async fn measurement(
    State(state): State<EvaluationWorkerState>, headers: HeaderMap, Json(input): Json<MeasurementSubmission>,
) -> Result<StatusCode, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    verify_worker(&worker, &input.lease)?;
    state.lifecycle.record_measurement(&worker.owner_id, &input.lease, &input.measurement).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CleanupSubmission { lease: ExecutionLease, container_id: Option<String>, network_id: Option<String>, succeeded: bool, error: Option<String> }

pub(super) async fn cleanup(
    State(state): State<EvaluationWorkerState>, headers: HeaderMap, Json(input): Json<CleanupSubmission>,
) -> Result<StatusCode, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?; verify_worker(&worker, &input.lease)?;
    state.lifecycle.record_cleanup(&worker.owner_id, &input.lease, input.container_id.as_deref(), input.network_id.as_deref(), input.succeeded, input.error.as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn reconcile(
    State(state): State<EvaluationWorkerState>, headers: HeaderMap,
) -> Result<Json<u64>, WorkerHttpError> {
    let worker = authenticate(&state, &headers).await?;
    Ok(Json(state.lifecycle.reconcile_restart(&worker.owner_id).await?))
}
