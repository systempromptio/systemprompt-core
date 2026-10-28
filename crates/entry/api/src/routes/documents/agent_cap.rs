//! `/api/v1/documents/{id}/agent-cap` and
//! `/api/v1/document-folders/{id}/agent-cap`: read and set the owner's
//! override on how far agents may go there. `GET` needs view; `PUT` needs
//! own and takes `{"cap": "none" | "read" | "comment" | "suggest" | "edit"
//! | null}`, where `null` clears the override. An override looser than the
//! agent policy allows is refused. `GET ?agent=<name>&attended=<bool>`
//! answers for that agent (default: an attended agent named `agent`) on
//! `channel=direct|drive_sync|live_checkpoint` (default `direct`), and
//! the answer always carries both the `attended` and `unattended` caps; a
//! name outside 1 to 64 of letters, digits, `-`, `_`, `.`, `:` or `/` is
//! refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::agents::{
    AgentActor, AgentCap, AgentChannel, CapSource, EffectiveCap,
};
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

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentQuery {
    agent: Option<String>,
    attended: Option<bool>,
    channel: Option<AgentChannel>,
}

const MAX_AGENT_NAME: usize = 64;

impl AgentQuery {
    fn actor(self) -> DocumentsResult<AgentActor> {
        let name = self.agent.unwrap_or_else(|| "agent".to_owned());
        let allowed =
            |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/');
        if name.is_empty() || name.len() > MAX_AGENT_NAME || !name.chars().all(allowed) {
            return Err(DocumentsError::InvalidParams {
                detail: "agent must be 1 to 64 letters, digits, '-', '_', '.', ':' or '/'",
            }
            .into());
        }
        Ok(AgentActor {
            name,
            attended: self.attended.unwrap_or(true),
            channel: self.channel.unwrap_or_default(),
        })
    }
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
    attended: CapJson,
    unattended: CapJson,
}

/// One case of the cap: for the agent with a person in its session, or
/// without.
#[derive(Debug, Serialize)]
pub(super) struct CapJson {
    agent_cap: AgentCap,
    policy_cap: AgentCap,
    source: CapSource,
    reason: String,
}

impl From<EffectiveCap> for CapJson {
    fn from(effective: EffectiveCap) -> Self {
        Self {
            agent_cap: effective.cap,
            policy_cap: effective.policy_cap,
            reason: effective.reason(),
            source: effective.source,
        }
    }
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
            attended: view.attended.into(),
            unattended: view.unattended.into(),
        }
    }
}

async fn read(
    state: &DocumentsState,
    req_ctx: &RequestContext,
    target: AgentCapTarget,
    query: AgentQuery,
) -> DocumentsResult<Json<AgentCapJson>> {
    let who = caller(state, req_ctx).await?;
    let agent = query.actor()?;
    let view = state
        .service
        .agent_cap_as(&who.user, target, agent, &who.trace)
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
    Query(query): Query<AgentQuery>,
) -> DocumentsResult<Json<AgentCapJson>> {
    let target = AgentCapTarget::Document(DocumentId::try_new(&id)?);
    read(&state, &req_ctx, target, query).await
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
    Query(query): Query<AgentQuery>,
) -> DocumentsResult<Json<AgentCapJson>> {
    let target = AgentCapTarget::Folder(DocumentFolderId::try_new(&id)?);
    read(&state, &req_ctx, target, query).await
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
