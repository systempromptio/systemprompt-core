use systemprompt_files::documents::codecs::MarkdownCodec;
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, NodeKind};

use super::{SAMPLE_MD, apply_str, hash_of, id, replace};

#[test]
fn projects_lines_and_block_outline() {
    let projection = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let heading = projection.find(&id("h[1]")).expect("heading");
    assert_eq!(heading.kind, NodeKind::Heading { level: 1 });
    assert_eq!(heading.text.as_deref(), Some("Title"));
    let para = projection.find(&id("p[1]")).expect("paragraph");
    assert_eq!(
        para.text.as_deref(),
        Some("First paragraph continues here.")
    );
    assert_eq!(para.attrs.get("lines").map(String::as_str), Some("3-4"));
    let item = projection.find(&id("li[2]")).expect("item");
    assert_eq!(item.text.as_deref(), Some("item two"));
    assert_eq!(
        projection
            .find(&id("line[6]"))
            .and_then(|n| n.text.clone())
            .as_deref(),
        Some("- item one")
    );
}

#[test]
fn line_edits_apply_and_blocks_are_read_only() {
    let base = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let out = apply_str(
        &MarkdownCodec,
        SAMPLE_MD,
        &[replace("line[1]", hash_of(&base, "line[1]"), "# New")],
    );
    assert!(out.starts_with("# New\n\nFirst paragraph\n"));
    assert_eq!(out.len(), SAMPLE_MD.len() - 2);
    let block = MarkdownCodec.apply(
        SAMPLE_MD,
        &base,
        &[replace("h[1]", hash_of(&base, "h[1]"), "x")],
    );
    assert!(matches!(block, Err(CodecError::UnsupportedOp { .. })));
}

#[test]
fn preview_strips_script() {
    let preview = MarkdownCodec.preview_html(SAMPLE_MD).expect("preview");
    assert!(preview.contains("<h1>Title</h1>"));
    assert!(!preview.contains("<script"));
}
