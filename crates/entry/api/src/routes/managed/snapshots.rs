//! Administrative snapshot reads and idempotent bounded asynchronous range
//! requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::ManagedHttpError;
use super::state::ManagedState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use systemprompt_analytics::snapshots::{
    FeedbackSnapshot, SnapshotHealth, SnapshotRangeJob, SnapshotRangeRequest,
};
use systemprompt_identifiers::{AnalyticsSnapshotJobId, ManagedResourceId};
use systemprompt_runtime::AppContext;

pub(super) fn router() -> Router<ManagedState> {
    Router::new()
        .route("/analytics/live", get(super::snapshot_stream::stream))
        .route("/analytics/snapshots", get(list))
        .route("/analytics/snapshots/portfolio", get(portfolio))
        .route("/analytics/snapshots/{resource}", get(resource))
        .route("/analytics/status", get(health))
        .route("/analytics/jobs", post(create_job))
        .route("/analytics/jobs/{operation}", get(job))
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Window {
    days: Option<u32>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Cursor {
    days: Option<u32>,
    after: Option<ManagedResourceId>,
    #[schemars(range(min = 1, max = 100))]
    limit: Option<u32>,
}
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct Page {
    items: Vec<FeedbackSnapshot>,
    next_cursor: Option<ManagedResourceId>,
}
async fn list(
    State(ctx): State<AppContext>,
    Query(query): Query<Cursor>,
) -> Result<Json<Page>, ManagedHttpError> {
    let limit = query.limit.unwrap_or(50);
    let items = ctx
        .feedback_snapshots_repository()
        .snapshots(
            ctx.system_admin().id(),
            query.after.as_ref(),
            query.days.unwrap_or(30),
            limit,
        )
        .await?;
    let next_cursor = if items.len() == limit as usize {
        items.last().and_then(|item| item.resource_id.clone())
    } else {
        None
    };
    Ok(Json(Page { items, next_cursor }))
}
async fn portfolio(
    State(ctx): State<AppContext>,
    Query(query): Query<Window>,
) -> Result<Json<Option<FeedbackSnapshot>>, ManagedHttpError> {
    Ok(Json(
        ctx.feedback_snapshots_repository()
            .snapshot(ctx.system_admin().id(), None, query.days.unwrap_or(30))
            .await?,
    ))
}
async fn resource(
    State(ctx): State<AppContext>,
    Path(resource): Path<ManagedResourceId>,
    Query(query): Query<Window>,
) -> Result<Json<Option<FeedbackSnapshot>>, ManagedHttpError> {
    Ok(Json(
        ctx.feedback_snapshots_repository()
            .snapshot(
                ctx.system_admin().id(),
                Some(&resource),
                query.days.unwrap_or(30),
            )
            .await?,
    ))
}
async fn health(
    State(ctx): State<AppContext>,
) -> Result<Json<SnapshotHealth>, ManagedHttpError> {
    Ok(Json(
        ctx.feedback_snapshots_repository()
            .health(ctx.system_admin().id())
            .await?,
    ))
}
async fn create_job(
    State(ctx): State<AppContext>,
    Json(request): Json<SnapshotRangeRequest>,
) -> Result<(StatusCode, Json<SnapshotRangeJob>), ManagedHttpError> {
    Ok((
        StatusCode::ACCEPTED,
        Json(
            ctx.feedback_snapshots_repository()
                .request_range(ctx.system_admin().id(), &request, chrono::Utc::now())
                .await?,
        ),
    ))
}
async fn job(
    State(ctx): State<AppContext>,
    Path(operation): Path<AnalyticsSnapshotJobId>,
) -> Result<Json<SnapshotRangeJob>, ManagedHttpError> {
    Ok(Json(
        ctx.feedback_snapshots_repository()
            .range_job(ctx.system_admin().id(), &operation)
            .await?
            .ok_or_else(|| {
                ManagedHttpError::NotFound("Analytics job unavailable".to_owned())
            })?,
    ))
}
