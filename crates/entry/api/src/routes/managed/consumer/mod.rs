//! Device credential consumer routes: enrollment, installation plans,
//! receipts and session bindings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod authorization;
mod error;

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use systemprompt_models::feedback::receipts::{ConsumerReceiptRequest, SessionBindingRequest};
use systemprompt_runtime::AppContext;

use error::ConsumerHttpError;

pub fn router() -> Router<AppContext> {
    Router::new()
        .route(
            "/consumer/resources/{resource}/publications/{publication}/bundle",
            get(bundle),
        )
        .route("/consumer-devices/enrollment", post(enroll))
        .route("/consumer/receipts", post(receipt))
        .route("/consumer/session-bindings", post(bind_session))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}

async fn receipt(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ConsumerReceiptRequest>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    Box::pin(authorization::resource(
        &ctx,
        credential,
        &input.resource_id,
        input.host,
    ))
    .await?;
    Ok(Json(
        ctx.managed_repository()
            .record_consumer_receipt(credential, &input)
            .await?,
    ))
}

async fn bind_session(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<SessionBindingRequest>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    let resource = ctx
        .managed_repository()
        .consumer_receipt_resource(credential, &input.receipt_id)
        .await?;
    Box::pin(authorization::resource(
        &ctx, credential, &resource, input.host,
    ))
    .await?;
    Ok(Json(
        ctx.managed_repository()
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
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    let identity = authorization::authenticate_device(&ctx, credential).await?;
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
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((resource, publication)): Path<(
        systemprompt_identifiers::ManagedResourceId,
        systemprompt_identifiers::PublicationId,
    )>,
    axum::extract::Query(query): axum::extract::Query<BundleQuery>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    Box::pin(authorization::resource(
        &ctx, credential, &resource, query.host,
    ))
    .await?;
    Ok(Json(
        ctx.managed_repository()
            .consumer_installation_plan(credential, &resource, &publication, query.host)
            .await?,
    ))
}
