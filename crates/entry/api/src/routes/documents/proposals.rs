//! `/api/v1/documents/{id}/proposals`: listing proposals, reading one with
//! the lines it changes against its base, and accepting or rejecting it.
//!
//! An edit whose mode resolves to `propose` answers `201 Created` with the
//! new proposal and its revision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::DocumentProposal;
use systemprompt_files::documents::service::{Proposed, ReviewRequest};
use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::documents::{ActorKind, ProposalStatus};
use systemprompt_runtime::documents::mcp::proposal_lines;

use super::DocumentsState;
use super::caller::{Caller, api_author, caller};
use super::error::DocumentsResult;
use super::items::document_id;
use super::views::RevisionJson;

#[derive(Debug, Serialize)]
pub(super) struct ProposalJson {
    id: DocumentProposalId,
    document_id: DocumentId,
    base_revision_id: DocumentRevisionId,
    proposed_revision_id: DocumentRevisionId,
    proposer: UserId,
    actor_kind: ActorKind,
    status: ProposalStatus,
    message: Option<String>,
    reviewer: Option<UserId>,
    reviewed_at: Option<DateTime<Utc>>,
    note: Option<String>,
    landed_revision_id: Option<DocumentRevisionId>,
    created_at: DateTime<Utc>,
}

impl From<DocumentProposal> for ProposalJson {
    fn from(p: DocumentProposal) -> Self {
        Self {
            id: p.id,
            document_id: p.document_id,
            base_revision_id: p.base_revision_id,
            proposed_revision_id: p.proposed_revision_id,
            proposer: p.proposer_user_id,
            actor_kind: p.actor_kind,
            status: p.status,
            message: p.message,
            reviewer: p.reviewer_user_id,
            reviewed_at: p.reviewed_at,
            note: p.note,
            landed_revision_id: p.landed_revision_id,
            created_at: p.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
struct ProposedJson {
    proposal: ProposalJson,
    revision: RevisionJson,
    superseded: Vec<DocumentProposalId>,
}

pub(super) fn proposed_response(proposed: Proposed) -> Response {
    let body = ProposedJson {
        proposal: proposed.proposal.into(),
        revision: proposed.revision.into(),
        superseded: proposed.superseded.into_iter().map(|p| p.id).collect(),
    };
    (StatusCode::CREATED, Json(body)).into_response()
}

#[derive(Debug, Deserialize)]
pub(super) struct ListQuery {
    #[serde(default)]
    status: Option<ProposalStatus>,
}

#[derive(Debug, Serialize)]
pub(super) struct ListJson {
    proposals: Vec<ProposalJson>,
}

pub(super) async fn list(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<ListQuery>,
) -> DocumentsResult<Json<ListJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let proposals = state
        .service
        .list_proposals(&who.user, &id, query.status, &who.trace)
        .await?;
    Ok(Json(ListJson {
        proposals: proposals.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Serialize)]
pub(super) struct DetailJson {
    proposal: ProposalJson,
    lines: Vec<String>,
}

fn ids(document: &str, proposal: &str) -> DocumentsResult<(DocumentId, DocumentProposalId)> {
    Ok((document_id(document)?, DocumentProposalId::try_new(proposal)?))
}

fn belongs(proposal: &DocumentProposal, document: &DocumentId) -> Result<(), DocumentsError> {
    if proposal.document_id == *document {
        return Ok(());
    }
    Err(DocumentsError::NotFound {
        what: "proposal",
        id: proposal.id.to_string(),
    })
}

pub(super) async fn get(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, pid)): Path<(String, String)>,
) -> DocumentsResult<Json<DetailJson>> {
    let who = caller(&state, &req_ctx).await?;
    let (id, pid) = ids(&id, &pid)?;
    let view = state
        .service
        .proposal_view(&who.user, &pid, &who.trace)
        .await?;
    belongs(&view.proposal, &id)?;
    Ok(Json(DetailJson {
        lines: proposal_lines(&view.base, &view.proposed),
        proposal: view.proposal.into(),
    }))
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ReviewBody {
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct ReviewedJson {
    proposal: ProposalJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<RevisionJson>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    rebased: bool,
}

async fn review_request(
    state: &DocumentsState,
    who: &Caller,
    (req_ctx, headers): (&RequestContext, &HeaderMap),
    (id, pid): (&DocumentId, DocumentProposalId),
    note: Option<String>,
) -> DocumentsResult<ReviewRequest> {
    let proposal = state.service.proposal(&who.user, &pid, &who.trace).await?;
    belongs(&proposal, id)?;
    Ok(ReviewRequest {
        proposal_id: pid,
        note,
        reviewer: api_author(req_ctx, headers),
    })
}

pub(super) async fn accept(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, pid)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<ReviewBody>,
) -> DocumentsResult<Json<ReviewedJson>> {
    let who = caller(&state, &req_ctx).await?;
    let (id, pid) = ids(&id, &pid)?;
    let request =
        review_request(&state, &who, (&req_ctx, &headers), (&id, pid), body.note).await?;
    let accepted = state
        .service
        .accept_proposal(&who.user, request, &who.trace)
        .await?;
    Ok(Json(ReviewedJson {
        proposal: accepted.proposal.into(),
        revision: Some(accepted.revision.into()),
        rebased: accepted.rebased,
    }))
}

pub(super) async fn reject(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, pid)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<ReviewBody>,
) -> DocumentsResult<Json<ReviewedJson>> {
    let who = caller(&state, &req_ctx).await?;
    let (id, pid) = ids(&id, &pid)?;
    let request =
        review_request(&state, &who, (&req_ctx, &headers), (&id, pid), body.note).await?;
    let rejected = state
        .service
        .reject_proposal(&who.user, request, &who.trace)
        .await?;
    Ok(Json(ReviewedJson {
        proposal: rejected.into(),
        revision: None,
        rebased: false,
    }))
}
