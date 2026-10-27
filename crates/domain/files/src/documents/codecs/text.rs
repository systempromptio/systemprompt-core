//! The plain-text codec.
//!
//! A text document projects to one `body` node whose children are the
//! 1-based `line[N]` lines. Edits keep the file's line-ending style
//! (`\r\n` or `\n`) and whether it ends with a newline; text an op
//! introduces is converted to that style.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, NodeKind, Projection, ProjectionNode,
    TemplateParams, content_hash, hash8,
};
use systemprompt_provider_contracts::register_document_codec;

use super::anchors::{decode_utf8, ensure_base};
use super::lines::{BODY_ID, LineDoc};
use super::placeholders::substitute;

#[derive(Debug, Clone, Copy, Default)]
pub struct TextCodec;

const FORMATS: &[DocumentFormat] = &[DocumentFormat::Text];

fn factory() -> Arc<dyn DocumentCodec> {
    Arc::new(TextCodec)
}

register_document_codec!(factory);

pub(crate) fn line_body(text: &str, kind: NodeKind) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(BODY_ID),
        kind,
        text: None,
        hash8: hash8(text),
        children: LineDoc::parse(text).line_nodes(),
        attrs: BTreeMap::new(),
    }
}

pub(crate) fn apply_lines(
    bytes: &[u8],
    base: &Projection,
    current: &Projection,
    ops: &[EditOp],
) -> Result<Vec<u8>, CodecError> {
    ensure_base(bytes, base)?;
    ops.iter().try_for_each(EditOp::check_resolved)?;
    let mut doc = LineDoc::parse(decode_utf8(bytes, base.format.as_str())?);
    for op in ops {
        doc.apply(op, base.format, current)?;
    }
    Ok(doc.render().into_bytes())
}

pub(crate) fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

impl DocumentCodec for TextCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["text"]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        FORMATS
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let text = decode_utf8(bytes, "text")?;
        Ok(Projection {
            format: DocumentFormat::Text,
            root: vec![line_body(text, NodeKind::Text)],
            content_sha256: content_hash(bytes),
        })
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        let current = self.project(bytes)?;
        apply_lines(bytes, base, &current, ops)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(format!(
            "<pre>{}</pre>",
            escape_html(decode_utf8(bytes, "text")?)
        ))
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(decode_utf8(bytes, "text")?.to_owned())
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        Ok(substitute(decode_utf8(template, "template")?, params).into_bytes())
    }
}
