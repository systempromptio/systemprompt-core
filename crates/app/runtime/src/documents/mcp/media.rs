//! The image bytes a write needs, resolved before it is committed.
//!
//! A `docx` embeds its images, so every blob reference a content model or
//! an op places (`insert_image`, an image `insert_block`) must be a
//! `blob_ref` the caller uploaded with `documents_upload`. Ownership is
//! checked through the uploads service, so a content hash alone never
//! places bytes someone else uploaded; the bytes then come from the blob
//! store. A reference that is already a URL or a picture the document
//! holds needs no bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::model::{
    Block, BlockKind, Media, MediaBlob, RichText, media_refs,
};
use systemprompt_identifiers::ContentHash;
use systemprompt_models::documents::EditOp;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::refusal::refusal;
use super::support::tool_error;
use super::upload::upload_error;
use crate::documents::refusal::RefusalCode;

/// Every image reference in `model` and `ops` that needs its bytes.
pub(super) fn image_refs(model: Option<&RichText>, ops: &[EditOp]) -> Vec<String> {
    let mut blocks = Vec::new();
    for op in ops {
        match op {
            EditOp::InsertImage(op) => {
                blocks.push(Block::new("", BlockKind::image(op.blob.clone(), String::new()), ""));
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

pub(super) async fn resolve_media(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    refs: Vec<String>,
) -> Result<Media, McpError> {
    let mut media = Media::new();
    if refs.is_empty() {
        return Ok(media);
    }
    let uploads = tools.uploads().ok_or_else(|| {
        refusal(
            RefusalCode::InvalidParams,
            "this server does not accept uploads, so it cannot place images",
        )
    })?;
    for reference in refs {
        let hash = ContentHash::try_new(&reference).map_err(|_invalid| {
            refusal(
                RefusalCode::InvalidParams,
                format!("image {reference} is not a blob_ref; upload it with documents_upload"),
            )
        })?;
        let origin = uploads
            .require_owned_blob(ctx.user_id(), &hash)
            .await
            .map_err(upload_error)?;
        let bytes = tools.service().media_bytes(&hash).await.map_err(tool_error)?;
        media.insert(
            reference,
            MediaBlob {
                bytes,
                mime: origin.mime,
            },
        );
    }
    Ok(media)
}
