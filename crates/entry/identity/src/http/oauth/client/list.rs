//! OAuth client listing endpoint with pagination.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use tracing::instrument;

use super::super::extractors::OAuthRepo;
use super::super::{OAuthHttpError, internal};
use super::ownership::listing_scope;
use systemprompt_models::api::PaginationParams;
use systemprompt_models::{PaginationInfo, RequestContext};

const STATUS_MAX_CHARS: usize = 50;

#[derive(Debug, thiserror::Error)]
#[error("status must be 1 to {STATUS_MAX_CHARS} characters, got {0}")]
struct StatusLengthError(usize);

#[derive(Debug, Deserialize)]
pub struct ListClientsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,

    pub status: Option<String>,
}

impl ListClientsQuery {
    fn validate(&self) -> Result<(), StatusLengthError> {
        match self.status.as_deref().map(|status| status.chars().count()) {
            Some(chars) if !(1..=STATUS_MAX_CHARS).contains(&chars) => {
                Err(StatusLengthError(chars))
            },
            _ => Ok(()),
        }
    }
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
        internal::rejected(
            OAuthHttpError::invalid_request("Invalid query parameters"),
            e,
        )
    })?;

    let page = query.pagination.page;
    let per_page = query.pagination.per_page;
    let offset = query.pagination.offset();
    let limit = query.pagination.limit();

    let scope = listing_scope(&req_ctx);
    let clients = repository
        .list_clients_paginated(&scope, limit, offset)
        .await?;
    let total = repository.count_clients(&scope).await?;

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
