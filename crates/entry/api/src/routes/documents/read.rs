//! `GET /{id}/read`: the read views over HTTP (outline, text, range, cells,
//! changes, node, search and meta), assembled by the same view code the
//! `documents_read` tool runs, with the same parameters: a range is
//! `from_node` and `to_node`, a cell read takes `sheet` and `cells`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::DocumentsError;
use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::views::{
    NodeRange, ReadInput, ReadOutput, ReadView, read_view,
};

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;
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
    #[serde(default)]
    from_node: Option<DocumentNodeId>,
    #[serde(default)]
    to_node: Option<DocumentNodeId>,
    #[serde(default)]
    sheet: Option<String>,
    #[serde(default)]
    cells: Option<String>,
    #[serde(default)]
    verbose: bool,
}

fn range(
    from_node: Option<DocumentNodeId>,
    to_node: Option<DocumentNodeId>,
) -> Result<Option<NodeRange>, DocumentsError> {
    match (from_node, to_node) {
        (Some(from_node), Some(to_node)) => Ok(Some(NodeRange { from_node, to_node })),
        (None, None) => Ok(None),
        _ => Err(DocumentsError::InvalidParams {
            detail: "a range needs both from_node and to_node",
        }),
    }
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
        range: range(query.from_node, query.to_node)?,
        sheet: query.sheet,
        cells: query.cells,
        revision_id: query.revision_id,
        since_revision: query.since_revision,
        node_id: query.node_id,
        text: query.text,
        verbose: query.verbose,
    };
    Ok(Json(
        read_view(&state.service, &who.user, input, &who.trace).await?,
    ))
}
