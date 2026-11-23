//! A document's public links: list, create and revoke. Creating returns the
//! link's token and path once; the token is not stored and cannot be shown
//! again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::models::PublicLink;
use systemprompt_files::documents::service::PublicLinkRequest;
use systemprompt_identifiers::PublicLinkId;
use systemprompt_models::RequestContext;
use systemprompt_models::modules::ApiPaths;

use super::caller::caller;
use super::error::DocumentsResult;
use super::items::document_id;
use super::{DocumentsState, step_up_of};

#[derive(Debug, Serialize)]
pub(super) struct PublicLinksJson {
    links: Vec<PublicLink>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PublicLinkBody {
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    days: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(super) struct CreatedPublicLinkJson {
    link: PublicLink,
    token: String,
    path: String,
}

pub(super) async fn list_public_links(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<PublicLinksJson>> {
    let who = caller(&state, &req_ctx).await?;
    let links = state
        .service
        .list_public_links(&who.user, &document_id(&id)?, &who.trace)
        .await?;
    Ok(Json(PublicLinksJson { links }))
}

pub(super) async fn create_public_link(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<PublicLinkBody>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let request = PublicLinkRequest {
        password: body.password,
        days: body.days,
    };
    let created = state
        .service
        .create_public_link(&who.user, &document_id(&id)?, request, &who.trace)
        .await?;
    let path = format!("{}/{}", ApiPaths::PUBLIC_DOCUMENT_LINKS, created.token);
    Ok((
        StatusCode::CREATED,
        Json(CreatedPublicLinkJson {
            link: created.link,
            token: created.token,
            path,
        }),
    )
        .into_response())
}

pub(super) async fn revoke_public_link(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, link_id)): Path<(String, String)>,
) -> DocumentsResult<StatusCode> {
    let who = caller(&state, &req_ctx).await?;
    let link = PublicLinkId::try_new(link_id)?;
    state
        .service
        .revoke_public_link(&who.user, &document_id(&id)?, &link, &who.trace)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) fn with_public_links(
    router: Router<DocumentsState>,
    state: &DocumentsState,
) -> Router<DocumentsState> {
    let stepped_up = step_up_of(state);
    router
        .route(
            "/{id}/public-links",
            get(list_public_links).merge(stepped_up(post(create_public_link))),
        )
        .route(
            "/{id}/public-links/{link_id}",
            stepped_up(delete(revoke_public_link)),
        )
}
