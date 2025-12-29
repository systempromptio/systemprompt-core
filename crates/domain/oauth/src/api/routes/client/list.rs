#![allow(unused_qualifications)]

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use tracing::instrument;
use validator::Validate;

use crate::repository::OAuthRepository;
use systemprompt_models::api::PaginationParams;
use systemprompt_models::{ApiError, CollectionResponse, PaginationInfo, RequestContext};
use systemprompt_runtime::AppContext;

#[derive(Debug, Deserialize, Validate)]
pub struct ListClientsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,

    #[validate(length(min = 1, max = 50))]
    pub status: Option<String>,
}

#[instrument(skip(ctx, req_ctx, query))]
pub async fn list_clients(
    Extension(req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
    Query(query): Query<ListClientsQuery>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(Arc::clone(ctx.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };

    if let Err(e) = query.validate() {
        tracing::info!(
            reason = "Validation error",
            requested_by = %req_ctx.auth.user_id,
            "OAuth clients list rejected - validation failed"
        );
        return ApiError::validation_error(format!("Invalid query parameters: {e}"), vec![])
            .into_response();
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
            let response = CollectionResponse::paginated(
                clients
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<crate::models::clients::api::OAuthClientResponse>>(),
                pagination,
            );
            response.into_response()
        },
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!(
                error = %e,
                requested_by = %req_ctx.auth.user_id,
                "OAuth clients list failed"
            );
            ApiError::internal_error(format!("Failed to list clients: {e}")).into_response()
        },
    }
}
