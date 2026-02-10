#![allow(unused_qualifications)]


use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use tracing::instrument;
use validator::Validate;

use super::super::responses::{bad_request, internal_error};
use systemprompt_models::api::PaginationParams;
use systemprompt_models::{PaginationInfo, RequestContext};
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::OAuthState;

#[derive(Debug, Deserialize, Validate)]
pub struct ListClientsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,

    #[validate(length(min = 1, max = 50))]
    pub status: Option<String>,
}

fn init_error(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "server_error",
            "error_description": format!("Repository initialization failed: {e}")
        })),
    )
        .into_response()
}

fn paginated_response<T: serde::Serialize>(items: Vec<T>, pagination: PaginationInfo) -> Response {
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

#[instrument(skip(state, req_ctx, query))]
pub async fn list_clients(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    Query(query): Query<ListClientsQuery>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(state.db_pool()) {
        Ok(r) => r,
        Err(e) => return init_error(e),
    };

    if let Err(e) = query.validate() {
        tracing::info!(
            reason = "Validation error",
            requested_by = %req_ctx.auth.user_id,
            "OAuth clients list rejected - validation failed"
        );
        return bad_request(format!("Invalid query parameters: {e}"));
    }

    let page = query.pagination.page;
    let per_page = query.pagination.per_page;
    let offset = query.pagination.offset();
    let limit = query.pagination.limit();

    let clients_result = repository.list_clients_paginated(limit, offset).await;
    let count_result = repository.count_clients().await;

    match (clients_result, count_result) {
        (Ok(clients), Ok(total)) => {
            tracing::info!(
                count = clients.len(),
                total = total,
                page = page,
                per_page = per_page,
                requested_by = %req_ctx.auth.user_id,
                "OAuth clients listed"
            );
            let pagination = PaginationInfo::new(total, page, per_page);
            let items: Vec<systemprompt_oauth::clients::api::OAuthClientResponse> =
                clients.into_iter().map(Into::into).collect();
            paginated_response(items, pagination)
        },
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!(
                error = %e,
                requested_by = %req_ctx.auth.user_id,
                "OAuth clients list failed"
            );
            internal_error(format!("Failed to list clients: {e}"))
        },
    }
}
