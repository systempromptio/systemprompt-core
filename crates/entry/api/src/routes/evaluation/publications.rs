//! Human publication review remains explicit after evaluation and source
//! checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::collections::Page;
use super::optimization_error::OptimizationHttpError;
use super::optimization_state::OptimizationState;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use systemprompt_identifiers::ManagedResourceId;
use systemprompt_marketplace::managed::{
    PublicationDecision, PublicationHistoryEntry, PublicationRequest,
};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct HistoryQuery {
    before: Option<i64>,
    #[schemars(range(min = 1, max = 100))]
    limit: Option<u32>,
}
pub(super) fn router() -> Router<OptimizationState> {
    Router::new()
        .route("/publications", post(review))
        .route("/resources/{id}/publications", get(history))
}
async fn review(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Json(input): Json<PublicationRequest>,
) -> Result<Json<PublicationDecision>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .review_and_publish(ctx.system_admin().id(), actor.user_id(), &input)
            .await?,
    ))
}
async fn history(
    State(ctx): State<AppContext>,
    Path(id): Path<ManagedResourceId>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Page<PublicationHistoryEntry>>, OptimizationHttpError> {
    let limit = query.limit.unwrap_or(50);
    let items = ctx
        .managed_repository()
        .publication_history_page(ctx.system_admin().id(), &id, query.before, limit)
        .await?;
    let next_cursor = if items.len() == limit as usize {
        items
            .last()
            .map(|item| item.decision.generation.to_string())
    } else {
        None
    };
    Ok(Json(Page { items, next_cursor }))
}
