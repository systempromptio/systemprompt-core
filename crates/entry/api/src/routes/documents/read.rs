//! `GET /{id}/read`: the `documents_read` views over HTTP (outline, text,
//! changes, node, search and meta), assembled by the same code the MCP tool
//! runs so both surfaces answer alike.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::mcp::{
    ReadInput, ReadOutput, ReadView, read_view, refusal_code,
};

use super::DocumentsState;
use super::caller::caller;
use super::error::{DocumentsResult, tool_failure};
use super::items::document_id;

#[derive(Debug, Deserialize)]
pub(super) struct ReadQuery {
    #[serde(default)]
    view: ReadView,
    #[serde(default)]
    since_revision: Option<DocumentRevisionId>,
    #[serde(default)]
    node_id: Option<DocumentNodeId>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    revision_id: Option<DocumentRevisionId>,
}

pub(super) async fn read(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<ReadQuery>,
) -> DocumentsResult<Json<ReadOutput>> {
    let who = caller(&state, &req_ctx).await?;
    let input = ReadInput {
        document_id: document_id(&id)?,
        view: query.view,
        range: None,
        sheet: None,
        cells: None,
        revision_id: query.revision_id,
        since_revision: query.since_revision,
        node_id: query.node_id,
        text: query.text,
        verbose: false,
    };
    let (output, _) = read_view(&state.service, &who.user, input, &who.trace)
        .await
        .map_err(|error| tool_failure(refusal_code(&error), error.message.to_string()))?;
    Ok(Json(output))
}
