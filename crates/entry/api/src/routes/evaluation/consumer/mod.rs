//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod admin;
pub use systemprompt_marketplace::managed::operations::CredentialIssueStatus;
mod authorization;
mod error;

pub(crate) use admin::router as admin_router;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use systemprompt_identifiers::InstallationReceiptId;
use systemprompt_marketplace::managed::consumer::ConsumerInvocationRequest;
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
        .route("/consumer/receipts/{id}", get(receipt_status))
        .route("/consumer/session-bindings", post(bind_session))
        .route("/consumer/session-bindings/{id}", get(session_status))
        .route("/consumer/invocations", post(invocation))
        .route("/consumer/invocations/{id}", get(invocation_status))
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
    let result = ctx
        .managed_repository()
        .record_consumer_receipt(credential, &input)
        .await?;
    Ok((
        StatusCode::OK,
        [(
            "location",
            format!("/api/v1/consumer/receipts/{}", result.receipt_id),
        )],
        Json(result),
    ))
}

async fn receipt_status(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<InstallationReceiptId>,
) -> Result<impl axum::response::IntoResponse, ConsumerHttpError> {
    let credential = authorization::credential(&headers)?;
    let result = ctx
        .managed_repository()
        .consumer_receipt_status(credential, &id)
        .await?;
    Ok(Json(result))
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

async fn invocation(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ConsumerInvocationRequest>,
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
            .record_consumer_invocation(credential, &input)
            .await?,
    ))
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub(crate) struct Enrollment {
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

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BundleQuery {
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

async fn session_status(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<systemprompt_identifiers::InstallationSessionBindingId>,
) -> Result<
    Json<systemprompt_marketplace::managed::consumer::ConsumerSessionBinding>,
    ConsumerHttpError,
> {
    Ok(Json(
        ctx.managed_repository()
            .consumer_session_status(authorization::credential(&headers)?, &id)
            .await?,
    ))
}
async fn invocation_status(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<systemprompt_identifiers::ResourceInvocationId>,
    axum::extract::Query(query): axum::extract::Query<BundleQuery>,
) -> Result<Json<systemprompt_marketplace::managed::consumer::ConsumerAttribution>, ConsumerHttpError>
{
    Ok(Json(
        ctx.managed_repository()
            .consumer_invocation_status(authorization::credential(&headers)?, &id, query.host)
            .await?,
    ))
}
