//! Input limits (`files.documents.limits`): the number of ops in one call,
//! the text one op carries, Markdown and whole-document text, comment
//! bodies, collaborative updates, cursor payloads, template parameters and
//! the inflated size of an Office zip package. Each refusal is
//! [`DocumentsError::TooLarge`] naming the input.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::{ContentAs, EditOp};
use systemprompt_provider_contracts::documents::TemplateParams;

use super::DocumentService;
#[cfg(feature = "office")]
use crate::documents::codecs::PackageCaps;
use crate::documents::error::DocumentsError;
use crate::documents::revision::CommitBody;

const MAX_TEMPLATE_PARAMS: u64 = 256;

fn within(what: &'static str, size: usize, limit: u64) -> Result<(), DocumentsError> {
    let size = u64::try_from(size).unwrap_or(u64::MAX);
    if size > limit {
        return Err(DocumentsError::TooLarge { what, size, limit });
    }
    Ok(())
}

fn op_text(op: &EditOp) -> Option<(bool, usize)> {
    match op {
        EditOp::ReplaceText {
            text, content_as, ..
        }
        | EditOp::InsertAfter {
            text, content_as, ..
        } => Some((*content_as == ContentAs::Markdown, text.len())),
        EditOp::AppendMarkdown { markdown } => Some((true, markdown.len())),
        EditOp::SetText { text } => Some((true, text.len())),
        EditOp::SetHtml { html } => Some((true, html.len())),
        EditOp::StrReplace { old_str, new_str } => Some((false, old_str.len().max(new_str.len()))),
        EditOp::InsertSlide { title, body, .. } => Some((
            false,
            title.as_ref().map_or(0, String::len) + body.iter().map(String::len).sum::<usize>(),
        )),
        _ => None,
    }
}

impl DocumentService {
    pub fn check_ops_limits(&self, ops: &[EditOp]) -> Result<(), DocumentsError> {
        let limits = &self.config.limits;
        within("ops in one call", ops.len(), limits.max_ops_per_call)?;
        for (markdown, size) in ops.iter().filter_map(op_text) {
            if markdown {
                within("markdown (bytes)", size, limits.max_markdown_bytes)?;
            } else {
                within("op text (bytes)", size, limits.max_op_text_bytes)?;
            }
        }
        Ok(())
    }

    pub(super) fn check_body_limits(&self, body: &CommitBody) -> Result<(), DocumentsError> {
        match body {
            CommitBody::Ops(ops) => self.check_ops_limits(ops),
            CommitBody::Bytes(bytes) => self.check_size(bytes.len()),
        }
    }

    pub(super) fn check_template_limits(
        &self,
        params: &TemplateParams,
        ops: &[EditOp],
    ) -> Result<(), DocumentsError> {
        self.check_ops_limits(ops)?;
        within(
            "template parameters",
            params.values.len(),
            MAX_TEMPLATE_PARAMS,
        )?;
        let limit = self.config.limits.max_op_text_bytes;
        for (key, value) in &params.values {
            within("template parameter (bytes)", key.len() + value.len(), limit)?;
        }
        Ok(())
    }

    #[cfg(feature = "office")]
    pub(super) const fn package_caps(&self) -> PackageCaps {
        let limits = &self.config.limits;
        PackageCaps {
            part_bytes: limits.max_package_part_bytes,
            inflated_bytes: limits.max_package_inflated_bytes,
            parts: limits.max_package_parts,
        }
    }

    pub fn check_comment_limit(&self, body: &str) -> Result<(), DocumentsError> {
        within(
            "comment body (bytes)",
            body.len(),
            self.config.limits.max_comment_bytes,
        )
    }

    pub fn check_collab_update_limit(&self, size: usize) -> Result<(), DocumentsError> {
        within(
            "collaborative update (bytes)",
            size,
            self.config.limits.max_collab_update_bytes,
        )
    }

    pub fn check_cursor_limit(&self, size: usize) -> Result<(), DocumentsError> {
        within("cursor (bytes)", size, self.config.limits.max_cursor_bytes)
    }
}
