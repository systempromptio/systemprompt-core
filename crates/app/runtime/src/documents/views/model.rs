//! The `model` read view: a text document as its `RichText` v2 model
//! (`ModelViewV2`, the shape `/api/v1/documents/schema` describes), so an
//! agent sees every style, table, image and section the hashlines flatten.
//!
//! Each top-level block that matches a node of the projection, in order,
//! by text (or, for a table, by kind), carries that node's id, so the ids
//! a model read answers are the ids `documents_edit` ops address. A block
//! the projection does not show keeps its model id (`bN`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::documents::model::formats::import_rich_text;
use systemprompt_files::documents::model::{BlockKind, ModelViewV2, RichText};
use systemprompt_files::documents::{DocumentService, DocumentsError};
use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentUri, NodeKind, Projection, ProjectionNode};

use super::types::{ReadInput, ReadOutput};

const LOOKAHEAD: usize = 64;

pub(super) async fn model_view(
    service: &DocumentService,
    who: &AuthenticatedUser,
    input: &ReadInput,
    trace: &TraceId,
) -> Result<ReadOutput, DocumentsError> {
    let (revision, projection) = service
        .project(who, &input.document_id, input.revision_id.as_ref(), trace)
        .await?;
    let (_, bytes) = service
        .read_bytes(who, &input.document_id, Some(&revision.id), trace)
        .await?;
    let mut model = import_rich_text(projection.format, &bytes).map_err(|_unsupported| {
        DocumentsError::InvalidParams {
            detail: "view model reads Markdown, HTML, docx and odt documents",
        }
    })?;
    name_blocks(&mut model, &projection);
    let view = serde_json::to_value(ModelViewV2::new(model)).map_err(|_unserialisable| {
        DocumentsError::InvalidParams {
            detail: "the document's model could not be serialised",
        }
    })?;
    Ok(ReadOutput {
        uri: input.verbose.then(|| {
            DocumentUri::new(input.document_id.clone()).at_revision(revision.id.clone())
        }),
        revision_id: revision.id,
        content_sha256: input.verbose.then_some(projection.content_sha256),
        format: projection.format,
        lines: Vec::new(),
        truncated: false,
        meta: None,
        paths: Vec::new(),
        model: Some(view),
    })
}

fn name_blocks(model: &mut RichText, projection: &Projection) {
    let nodes: Vec<&ProjectionNode> = projection.root.iter().collect();
    let mut cursor = 0;
    for block in &mut model.blocks {
        let table = matches!(block.kind, BlockKind::Table { .. });
        let found = nodes
            .iter()
            .enumerate()
            .skip(cursor)
            .take(LOOKAHEAD)
            .find(|(_, node)| {
                if table {
                    node.kind == NodeKind::Table
                } else {
                    node.kind != NodeKind::Table
                        && plain(node.text.as_deref().unwrap_or_default()) == plain(&block.text)
                }
            });
        if let Some((at, node)) = found {
            block.id = node.id.as_str().to_owned();
            cursor = at + 1;
        }
    }
}

fn plain(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
