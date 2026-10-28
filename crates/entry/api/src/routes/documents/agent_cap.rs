//! `/api/v1/documents/{id}/agent-cap` and
//! `/api/v1/document-folders/{id}/agent-cap`: read and set the owner's
//! override on how far agents may go there. `GET` needs view; `PUT` needs
//! own and takes `{"cap": "none" | "read" | "comment" | "suggest" | "edit"
//! | null}`, where `null` clears the override. An override looser than the
//! agent policy allows is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::agents::{AgentCap, CapSource};
use systemprompt_files::documents::service::{AgentCapTarget, AgentCapView};
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::RequestContext;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentCapBody {
    cap: Option<AgentCap>,
}

#[derive(Debug, Serialize)]
pub(super) struct AgentCapJson {
    target_kind: &'static str,
    target_id: String,
    #[serde(rename = "override")]
    own_override: Option<AgentCap>,
    agent_cap: AgentCap,
    policy_cap: AgentCap,
    source: CapSource,
    reason: String,
}

impl From<AgentCapView> for AgentCapJson {
    fn from(view: AgentCapView) -> Self {
        let (target_kind, target_id) = match view.target {
            AgentCapTarget::Document(id) => ("document", id.to_string()),
            AgentCapTarget::Folder(id) => ("folder", id.to_string()),
        };
        Self {
            target_kind,
            target_id,
            own_override: view.own_override,
            agent_cap: view.effective.cap,
            policy_cap: view.effective.policy_cap,
            reason: view.effective.reason(),
            source: view.effective.source,
        }
    }
}

async fn read(
    state: &DocumentsState,
    req_ctx: &RequestContext,
    target: AgentCapTarget,
) -> DocumentsResult<Json<AgentCapJson>> {
    let who = caller(state, req_ctx).await?;
    let view = state
        .service
        .agent_cap(&who.user, target, &who.trace)
        .await?;
    Ok(Json(view.into()))
}

async fn write(
    state: &DocumentsState,
    req_ctx: &RequestContext,
    target: AgentCapTarget,
    body: AgentCapBody,
) -> DocumentsResult<Json<AgentCapJson>> {
    let who = caller(state, req_ctx).await?;
    let view = state
        .service
        .set_agent_cap(&who.user, target, body.cap, &who.trace)
        .await?;
    Ok(Json(view.into()))
}

pub(super) async fn get_document(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<AgentCapJson>> {
    let target = AgentCapTarget::Document(DocumentId::try_new(&id)?);
    read(&state, &req_ctx, target).await
}

pub(super) async fn put_document(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<AgentCapBody>,
) -> DocumentsResult<Json<AgentCapJson>> {
    let target = AgentCapTarget::Document(DocumentId::try_new(&id)?);
    write(&state, &req_ctx, target, body).await
}

pub(super) async fn get_folder(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<AgentCapJson>> {
    let target = AgentCapTarget::Folder(DocumentFolderId::try_new(&id)?);
    read(&state, &req_ctx, target).await
}

pub(super) async fn put_folder(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<AgentCapBody>,
) -> DocumentsResult<Json<AgentCapJson>> {
    let target = AgentCapTarget::Folder(DocumentFolderId::try_new(&id)?);
    write(&state, &req_ctx, target, body).await
}
