//! Device credential consumer routes: enrollment, installation plans,
//! receipts and session bindings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod authorization;
mod error;

pub use authorization::instance_serves;

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use systemprompt_models::ApiPaths;
use systemprompt_models::feedback::receipts::{ConsumerReceiptRequest, SessionBindingRequest};

use super::ManagedConsumerState;
use error::ConsumerHttpError;

pub fn router() -> Router<ManagedConsumerState> {
    Router::new()
        .route(
            ApiPaths::api_v1_relative(ApiPaths::CONSUMER_BUNDLE_ROUTE),
            get(bundle),
        )
        .route(
            ApiPaths::api_v1_relative(ApiPaths::CONSUMER_DEVICE_ENROLLMENT),
            post(enroll),
        )
        .route(
            ApiPaths::api_v1_relative(ApiPaths::CONSUMER_RECEIPTS),
            post(receipt),
        )
        .route(
            ApiPaths::api_v1_relative(ApiPaths::CONSUMER_SESSION_BINDINGS),
            post(bind_session),
        )
        .layer(DefaultBodyLimit::max(1024 * 1024))
}

async fn receipt(
    State(state): State<ManagedConsumerState>,
    headers: HeaderMap,
    Json(input): Json<ConsumerReceiptRequest>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    Box::pin(authorization::resource(
        &state,
        credential,
        &input.resource_id,
        input.host,
    ))
    .await?;
    Ok(Json(
        state
            .repository
            .record_consumer_receipt(credential, &input)
            .await?,
    ))
}

async fn bind_session(
    State(state): State<ManagedConsumerState>,
    headers: HeaderMap,
    Json(input): Json<SessionBindingRequest>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    let resource = state
        .repository
        .consumer_receipt_resource(credential, &input.receipt_id)
        .await?;
    Box::pin(authorization::resource(
        &state, credential, &resource, input.host,
    ))
    .await?;
    Ok(Json(
        state
            .repository
            .bind_consumer_session(credential, &input)
            .await?,
    ))
}

#[derive(serde::Serialize)]
struct Enrollment {
    device_id: systemprompt_identifiers::DeviceId,
    consumer_id: systemprompt_identifiers::UserId,
}

async fn enroll(
    State(state): State<ManagedConsumerState>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    let identity = authorization::authenticate_device(&state, credential).await?;
    Ok(Json(Enrollment {
        device_id: identity.device_id,
        consumer_id: identity.consumer_id,
    }))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleQuery {
    host: systemprompt_models::feedback::EvaluatorClient,
}

async fn bundle(
    State(state): State<ManagedConsumerState>,
    headers: HeaderMap,
    Path((resource, publication)): Path<(
        systemprompt_identifiers::ManagedResourceId,
        systemprompt_identifiers::PublicationId,
    )>,
    axum::extract::Query(query): axum::extract::Query<BundleQuery>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    Box::pin(authorization::resource(
        &state, credential, &resource, query.host,
    ))
    .await?;
    Ok(Json(
        state
            .repository
            .consumer_installation_plan(credential, &resource, &publication, query.host)
            .await?,
    ))
}
