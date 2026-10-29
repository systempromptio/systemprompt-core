mod agent_caps;
mod batch_order;
mod blob_ids;
mod codecs_docx;
mod codecs_docx_comments;
mod codecs_docx_edits;
mod codecs_docx_inline;
mod codecs_fragments;
mod codecs_html;
mod codecs_html_paths;
mod codecs_markdown;
mod codecs_markdown_content;
mod codecs_odp;
mod codecs_ods;
mod codecs_odt;
mod codecs_package_limits;
mod codecs_pending;
mod codecs_pptx;
mod codecs_pptx_edits;
mod codecs_pptx_markdown;
mod codecs_pptx_slides;
mod codecs_text;
mod codecs_xlsx;
mod codecs_xlsx_paths;
mod collab_model;
mod commit_plan;
mod docx_fixture;
mod engine_loro;
mod engine_loro_model;
mod live_resume;
mod model_docx_review;
mod model_docx_roundtrip;
mod model_markdown_gfm;
mod model_office;
mod model_ops;
mod model_rich_text;
mod model_style_merge;
mod node_ids;
mod odf_fixture;
mod pdf_render;
mod pptx_fixture;
mod registry;
mod targets;
mod templates;
mod templates_catalog_errors;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget, Projection,
};

pub(crate) const SAMPLE_TXT: &[u8] = include_bytes!("../../fixtures/documents/sample.txt");
pub(crate) const CRLF_TXT: &[u8] = include_bytes!("../../fixtures/documents/crlf.txt");
pub(crate) const SAMPLE_MD: &[u8] = include_bytes!("../../fixtures/documents/sample.md");
pub(crate) const SAMPLE_HTML: &[u8] = include_bytes!("../../fixtures/documents/sample.html");

pub(crate) fn id(value: &str) -> DocumentNodeId {
    DocumentNodeId::new(value)
}

pub(crate) fn hash_of(projection: &Projection, node: &str) -> String {
    projection
        .find(&id(node))
        .map(|found| found.hash8.clone())
        .unwrap_or_default()
}

pub(crate) fn replace(node: &str, expected_hash: String, text: &str) -> EditOp {
    EditOp::ReplaceText {
        node: NodeTarget::Id(id(node)),
        expected_hash: Some(expected_hash),
        text: text.to_owned(),
        content_as: ContentAs::Text,
    }
}

pub(crate) fn insert_after(node: &str, kind: NodeKind, text: &str) -> EditOp {
    EditOp::InsertAfter {
        node: NodeTarget::Id(id(node)),
        kind,
        text: text.to_owned(),
        content_as: ContentAs::Text,
    }
}

pub(crate) fn delete(node: &str, expected_hash: String) -> EditOp {
    EditOp::Delete {
        node: NodeTarget::Id(id(node)),
        expected_hash: Some(expected_hash),
    }
}

pub(crate) fn apply_str(codec: &dyn DocumentCodec, bytes: &[u8], ops: &[EditOp]) -> String {
    let base = codec.project(bytes).expect("project");
    let out = codec.apply(bytes, &base, ops).expect("apply");
    String::from_utf8(out).expect("utf8")
}
