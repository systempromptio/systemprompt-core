//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use systemprompt_identifiers::{DeviceCertId, ManagedResourceId, UserId};
use systemprompt_runtime::AppContext;

use super::error::ConsumerHttpError;

pub(crate) fn router() -> Router<AppContext> {
    Router::new()
        .route("/consumer-devices/{id}/credential", post(issue))
        .route("/consumer-devices/{id}/revocation", post(revoke))
        .route("/resources/{id}/consumer-grants", post(grant))
}

async fn issue(
    State(ctx): State<AppContext>,
    Path(id): Path<DeviceCertId>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .issue_consumer_credential(&id)
            .await?,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
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
