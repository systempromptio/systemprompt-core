mod codecs_html;
mod codecs_markdown;
mod codecs_text;
mod registry;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{DocumentCodec, EditOp, NodeKind, Projection};

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
        node: id(node),
        expected_hash,
        text: text.to_owned(),
    }
}

pub(crate) fn insert_after(node: &str, kind: NodeKind, text: &str) -> EditOp {
    EditOp::InsertAfter {
        node: id(node),
        kind,
        text: text.to_owned(),
    }
}

pub(crate) fn delete(node: &str, expected_hash: String) -> EditOp {
    EditOp::Delete {
        node: id(node),
        expected_hash,
    }
}

pub(crate) fn apply_str(codec: &dyn DocumentCodec, bytes: &[u8], ops: &[EditOp]) -> String {
    let base = codec.project(bytes).expect("project");
    let out = codec.apply(bytes, &base, ops).expect("apply");
    String::from_utf8(out).expect("utf8")
}
