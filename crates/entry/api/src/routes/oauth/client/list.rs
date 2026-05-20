#![allow(unused_qualifications)]

use axum::extract::{Extension, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use tracing::instrument;
use validator::Validate;

use super::super::OAuthHttpError;
use super::super::extractors::OAuthRepo;
use systemprompt_models::api::PaginationParams;
use systemprompt_models::{PaginationInfo, RequestContext};

#[derive(Debug, Deserialize, Validate)]
pub struct ListClientsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,

    #[validate(length(min = 1, max = 50))]
    pub status: Option<String>,
}

fn paginated_response<T: serde::Serialize>(items: &[T], pagination: &PaginationInfo) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "data": items,
            "meta": {
                "pagination": pagination
            }
        })),
    )
        .into_response()
}

#[instrument(skip(repository, req_ctx, query))]
pub async fn list_clients(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Query(query): Query<ListClientsQuery>,
) -> Result<Response, OAuthHttpError> {
    query.validate().map_err(|e| {
        OAuthHttpError::invalid_request(format!("Invalid query parameters: {e}"))
    })?;

    let page = query.pagination.page;
    let per_page = query.pagination.per_page;
    let offset = query.pagination.offset();
    let limit = query.pagination.limit();

    let clients = repository.list_clients_paginated(limit, offset).await?;
    let total = repository.count_clients().await?;

    tracing::info!(
        count = clients.len(),
        total = total,
        page = page,
        per_page = per_page,
        requested_by = %req_ctx.auth.actor.user_id,
        "OAuth clients listed"
    );
    let pagination = PaginationInfo::new(total, page, per_page);
    let items: Vec<systemprompt_oauth::clients::api::OAuthClientResponse> =
        clients.into_iter().map(Into::into).collect();
    Ok(paginated_response(&items, &pagination))
}
