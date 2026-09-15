//! Idempotent administrative requests expose retained status and typed results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::optimization_error::OptimizationHttpError;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TaskId;
use systemprompt_marketplace::managed::operations::{ApiOperation, ApiOperationClaim};
use systemprompt_runtime::AppContext;

/// Stable mutation status; successful repeats never execute its side effects
/// twice.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationStatus {
    pub id: TaskId,
    pub kind: String,
    pub state: String,
    pub problem: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
impl From<&ApiOperation> for OperationStatus {
    fn from(value: &ApiOperation) -> Self {
        Self {
            id: value.id.clone(),
            kind: value.kind.clone(),
            state: value.state.clone(),
            problem: value.problem.clone(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OperationResponse<T> {
    pub operation: OperationStatus,
    pub result: Option<T>,
}
/// The status endpoint discriminates every supported operation result.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OperationResult {
    ApprovalDecision(systemprompt_evaluation::repository::experiments::ExecutionApproval),
    InventoryRefresh(systemprompt_marketplace::inventory::InventoryStatus),
    SourceCapture(systemprompt_marketplace::managed::ImportedSkills),
    SourceVerification(systemprompt_models::feedback::verification::DependencyVerificationManifest),
    CredentialIssue(super::consumer::CredentialIssueStatus),
}
pub(super) fn router() -> Router<AppContext> {
    Router::new().route("/operations/{id}", get(status))
}
pub(super) fn key(headers: &HeaderMap) -> Result<TaskId, OptimizationHttpError> {
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|key| {
            !key.is_empty()
                && key.len() <= 200
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
        })
        .ok_or_else(|| {
            systemprompt_evaluation::EvaluationError::InvalidSpec(
                "Idempotency-Key header of 1–200 bytes is required".to_owned(),
            )
        })?;
    Ok(TaskId::new(key))
}
pub(super) async fn begin<T: Serialize + Sync>(
    ctx: &AppContext,
    headers: &HeaderMap,
    kind: &str,
    input: &T,
) -> Result<ApiOperationClaim, OptimizationHttpError> {
    Ok(ctx
        .managed_repository()
        .begin_api_operation(ctx.system_admin().id(), &key(headers)?, kind, input)
        .await?)
}
pub(super) fn response<T: serde::de::DeserializeOwned>(
    operation: &ApiOperation,
) -> Result<OperationResponse<T>, OptimizationHttpError> {
    let result = operation
        .result
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(systemprompt_evaluation::EvaluationError::from)?;
    Ok(OperationResponse {
        operation: OperationStatus::from(operation),
        result,
    })
}
pub(super) async fn finish<T: Serialize + serde::de::DeserializeOwned + Sync>(
    ctx: &AppContext,
    operation: &ApiOperation,
    result: Result<T, OptimizationHttpError>,
) -> Result<OperationResponse<T>, OptimizationHttpError> {
    match result {
        Ok(result) => {
            ctx.managed_repository()
                .finish_api_operation(ctx.system_admin().id(), operation, &result)
                .await?;
            response(
                &ctx.managed_repository()
                    .api_operation(ctx.system_admin().id(), &operation.id)
                    .await?,
            )
        },
        Err(error) => {
            ctx.managed_repository()
                .fail_api_operation(ctx.system_admin().id(), operation)
                .await?;
            Err(error)
        },
    }
}
async fn status(
    State(ctx): State<AppContext>,
    Path(id): Path<TaskId>,
) -> Result<Json<OperationResponse<OperationResult>>, OptimizationHttpError> {
    let operation = ctx
        .managed_repository()
        .api_operation(ctx.system_admin().id(), &id)
        .await?;
    let result = operation
        .result
        .clone()
        .map(
            |value| -> Result<OperationResult, systemprompt_evaluation::EvaluationError> {
                Ok(match operation.kind.as_str() {
                    "inventory_refresh" => {
                        OperationResult::InventoryRefresh(serde_json::from_value(value)?)
                    },
                    "source_capture" => {
                        OperationResult::SourceCapture(serde_json::from_value(value)?)
                    },
                    "source_verification" => {
                        OperationResult::SourceVerification(serde_json::from_value(value)?)
                    },
                    "approval_decision" => {
                        OperationResult::ApprovalDecision(serde_json::from_value(value)?)
                    },
                    "credential_issue" => {
                        OperationResult::CredentialIssue(serde_json::from_value(value)?)
                    },
                    _ => {
                        return Err(systemprompt_evaluation::EvaluationError::ResourceNotFound(
                            "Operation kind unavailable".to_owned(),
                        ));
                    },
                })
            },
        )
        .transpose()?;
    Ok(Json(OperationResponse {
        operation: OperationStatus::from(&operation),
        result,
    }))
}
pub(super) fn status_code<T>(response: &OperationResponse<T>) -> StatusCode {
    if response.operation.state == "pending" {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    }
}
