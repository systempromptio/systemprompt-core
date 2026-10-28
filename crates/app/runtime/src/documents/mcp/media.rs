//! The image bytes a write needs, resolved before it is committed.
//!
//! Every uploaded image a content model or an op places (`insert_image`,
//! an image `insert_block`) must be a `blob_ref` the caller uploaded with
//! `documents_upload`, or one the target document already holds: a hash
//! is visible to every viewer and is not a capability. The rule is
//! `DocumentService::admit_media`, the same one a live checkpoint applies.
//! A reference that is a URL or a picture inside the document needs no
//! bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::model::{Block, BlockKind, Media, RichText, media_refs};
use systemprompt_files::documents::service::held_media_refs;
use systemprompt_identifiers::DocumentId;
use systemprompt_models::documents::EditOp;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::refusal::refusal;
use super::support::{caller, tool_error};
use crate::documents::refusal::RefusalCode;

/// Every image reference in `model` and `ops` that needs its bytes.
pub(super) fn image_refs(model: Option<&RichText>, ops: &[EditOp]) -> Vec<String> {
    let mut blocks = Vec::new();
    for op in ops {
        match op {
            EditOp::InsertImage(op) => {
                blocks.push(Block::new(
                    "",
                    BlockKind::image(op.blob.clone(), String::new()),
                    "",
                ));
            },
            EditOp::InsertBlock(op) => {
                let mut value = op.block.clone();
                if let Some(object) = value.as_object_mut() {
                    object.insert("id".to_owned(), serde_json::Value::String(String::new()));
                }
                if let Ok(block) = serde_json::from_value::<Block>(value) {
                    blocks.push(block);
                }
            },
            _ => {},
        }
    }
    let mut refs = media_refs(&RichText::of(blocks));
    if let Some(model) = model {
        refs.extend(media_refs(model));
    }
    refs.sort();
    refs.dedup();
    refs
}

/// The bytes of the images a write places, admitted for `document` (none
/// for a create): each must be the caller's own upload or already held by
/// the document's head. Anything else refuses the write, naming the image.
pub(super) async fn resolve_media(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    refs: Vec<String>,
    document: Option<&DocumentId>,
) -> Result<Media, McpError> {
    if refs.is_empty() {
        return Ok(Media::new());
    }
    let service = tools.service();
    let held = match document {
        Some(document) => {
            let who = caller(ctx)?;
            let format = service
                .get(who, document, ctx.trace_id())
                .await
                .map_err(tool_error)?
                .document
                .format;
            let (_, bytes) = service
                .read_bytes(who, document, None, ctx.trace_id())
                .await
                .map_err(tool_error)?;
            held_media_refs(format, &bytes, &refs, service.config().max_upload_bytes)
        },
        None => Vec::new(),
    };
    let admitted = service
        .admit_media(
            tools.uploads(),
            &|_| vec![ctx.user_id().clone()],
            refs,
            &held,
        )
        .await;
    match admitted.refused.first() {
        Some(refused) => Err(refusal(
            RefusalCode::InvalidParams,
            format!(
                "image {refused} is neither your upload nor already in this document; upload it \
                 with documents_upload and pass its blob_ref"
            ),
        )),
        None => Ok(admitted.media),
    }
}
