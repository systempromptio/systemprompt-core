//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use systemprompt_identifiers::{DeviceCertId, ManagedResourceId, UserId};
use systemprompt_runtime::AppContext;

use super::super::state::ManagedState;
use super::error::ConsumerHttpError;

pub(crate) fn router() -> Router<ManagedState> {
    Router::new()
        .route("/consumer-devices/{id}/credential", post(issue))
        .route("/consumer-devices/{id}/revocation", post(revoke))
        .route("/resources/{id}/consumer-grants", post(grant))
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub(crate) struct CredentialIssueResponse {
    pub operation: super::super::operations::OperationStatus,
    pub result: Option<super::CredentialIssueStatus>,
    pub credential: Option<String>,
}
async fn issue(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<DeviceCertId>,
) -> Result<impl axum::response::IntoResponse, super::super::error::ManagedHttpError> {
    use systemprompt_marketplace::managed::operations::ApiOperationClaim;
    let claim = super::super::operations::begin(&ctx, &headers, "credential_issue", &id).await?;
    let response = match claim {
        ApiOperationClaim::Retained(operation) => {
            let result = operation
                .result
                .clone()
                .map(serde_json::from_value)
                .transpose()?;
            CredentialIssueResponse {
                operation: super::super::operations::OperationStatus::from(&operation),
                result,
                credential: None,
            }
        },
        ApiOperationClaim::Acquired(operation) => {
            let result = ctx
                .managed_repository()
                .issue_api_consumer_credential(ctx.system_admin().id(), &operation, &id)
                .await;
            let (status, credential) = match result {
                Ok(value) => value,
                Err(error) => {
                    ctx.managed_repository()
                        .fail_api_operation(ctx.system_admin().id(), &operation)
                        .await?;
                    return Err(error.into());
                },
            };
            let operation = ctx
                .managed_repository()
                .api_operation(ctx.system_admin().id(), &operation.id)
                .await?;
            CredentialIssueResponse {
                operation: super::super::operations::OperationStatus::from(&operation),
                result: Some(status),
                credential,
            }
        },
    };
    let status = if response.operation.state == "pending" {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        [(
            "location",
            format!("/api/v1/operations/{}", response.operation.id),
        )],
        Json(response),
    ))
}

async fn revoke(
    State(ctx): State<AppContext>,
    Path(id): Path<DeviceCertId>,
) -> Result<StatusCode, ConsumerHttpError> {
    ctx.managed_repository()
        .revoke_consumer_credential(&id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Grant {
    consumer_id: UserId,
    enabled: bool,
}

async fn grant(
    State(ctx): State<AppContext>,
    Path(id): Path<ManagedResourceId>,
    Json(input): Json<Grant>,
) -> Result<StatusCode, ConsumerHttpError> {
    ctx.managed_repository()
        .set_consumer_grant(
            ctx.system_admin().id(),
            &id,
            &input.consumer_id,
            input.enabled,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
