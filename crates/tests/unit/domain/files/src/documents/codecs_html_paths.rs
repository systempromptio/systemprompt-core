use systemprompt_files::documents::codecs::HtmlCodec;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget,
};

use super::{SAMPLE_HTML, apply_str, delete, hash_of, id, insert_after, replace};

fn apply(ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let base = HtmlCodec.project(SAMPLE_HTML).expect("project");
    HtmlCodec.apply(SAMPLE_HTML, &base, ops)
}

fn str_replace(old: &str, new: &str) -> EditOp {
    EditOp::StrReplace {
        old_str: old.to_owned(),
        new_str: new.to_owned(),
    }
}

#[test]
fn str_replace_rewrites_body_once() {
    let out = apply_str(
        &HtmlCodec,
        SAMPLE_HTML,
        &[str_replace("Second para", "Swapped")],
    );
    assert!(out.contains("Swapped"));
    assert!(!out.contains("Second para"));
    assert!(out.contains("<title>Keep Me</title>"));
}

#[test]
fn anchored_op_after_whole_document_op_is_refused() {
    let base = HtmlCodec.project(SAMPLE_HTML).expect("project");
    let err = apply(&[
        str_replace("Second para", "Swapped"),
        delete("body/p[1]", hash_of(&base, "body/p[1]")),
    ])
    .expect_err("ordering");
    assert!(matches!(err, CodecError::Malformed { .. }));
}

#[test]
fn replace_on_table_or_image_is_unsupported() {
    let base = HtmlCodec.project(SAMPLE_HTML).expect("project");
    for node in ["body/table[1]", "body/img[1]"] {
        let err = apply(&[replace(node, hash_of(&base, node), "x")]).expect_err("unsupported");
        assert!(
            matches!(
                err,
                CodecError::UnsupportedOp {
                    op: "replace_text",
                    ..
                }
            ),
            "{node}"
        );
    }
}

#[test]
fn unknown_node_is_reported() {
    let err = apply(&[delete("body/p[99]", "00000000".to_owned())]).expect_err("unknown");
    assert!(matches!(err, CodecError::UnknownNode { node } if node.as_str() == "body/p[99]"));
}

#[test]
fn unsupported_ops_name_themselves() {
    let err = apply(&[EditOp::SetText {
        text: "x".to_owned(),
    }])
    .expect_err("unsupported");
    assert!(matches!(
        err,
        CodecError::UnsupportedOp { op: "set_text", .. }
    ));
}

#[test]
fn insert_kinds_map_to_tags_and_others_are_unsupported() {
    let out = apply_str(
        &HtmlCodec,
        SAMPLE_HTML,
        &[
            insert_after("body/li[1]", NodeKind::ListItem, "inserted item"),
            insert_after("body/p[2]", NodeKind::Heading { level: 9 }, "Clamped"),
        ],
    );
    assert!(out.contains("<li>one</li><li>inserted item</li>"));
    assert!(out.contains("<h6>Clamped</h6>"));
    let err = apply(&[insert_after("body/p[1]", NodeKind::Image, "x")]).expect_err("image");
    assert!(matches!(
        err,
        CodecError::UnsupportedOp {
            op: "insert_after",
            ..
        }
    ));
}

#[test]
fn insert_after_body_prepends_and_chains() {
    let out = apply_str(
        &HtmlCodec,
        SAMPLE_HTML,
        &[
            insert_after("body", NodeKind::Paragraph, "first"),
            insert_after("body", NodeKind::Paragraph, "second"),
        ],
    );
    let first = out.find("<p>first</p>").expect("first");
    let second = out.find("<p>second</p>").expect("second");
    let heading = out.find("Heading").expect("heading");
    assert!(first < second && second < heading, "{out}");
}

#[test]
fn markdown_insert_splits_pending_markers() {
    let out = apply_str(
        &HtmlCodec,
        SAMPLE_HTML,
        &[EditOp::InsertAfter {
            node: NodeTarget::Id(id("body/p[2]")),
            kind: NodeKind::Paragraph,
            text: "**lead**\n<!-- pending: write the summary -->\ntail".to_owned(),
            content_as: ContentAs::Markdown,
        }],
    );
    assert!(out.contains("<strong>lead</strong>"));
    assert!(out.contains("<em>write the summary</em>"));
    assert!(out.contains("tail"));
    let projection = HtmlCodec.project(out.as_bytes()).expect("reproject");
    let pending: Vec<_> = projection
        .root
        .iter()
        .flat_map(|n| std::iter::once(n).chain(n.children.iter()))
        .filter(|n| n.kind == NodeKind::Pending)
        .collect();
    assert_eq!(pending.len(), 1);
}

#[test]
fn replacing_a_pending_node_fills_it() {
    let staged = apply_str(
        &HtmlCodec,
        SAMPLE_HTML,
        &[insert_after("body/p[2]", NodeKind::Pending, "draft intro")],
    );
    let base = HtmlCodec.project(staged.as_bytes()).expect("project");
    let pending = base
        .root
        .iter()
        .flat_map(|n| std::iter::once(n).chain(n.children.iter()))
        .find(|n| n.kind == NodeKind::Pending)
        .expect("pending")
        .clone();
    let out = HtmlCodec
        .apply(
            staged.as_bytes(),
            &base,
            &[EditOp::ReplaceText {
                node: NodeTarget::Id(pending.id.clone()),
                expected_hash: Some(pending.hash8.clone()),
                text: "The intro".to_owned(),
                content_as: ContentAs::Text,
            }],
        )
        .expect("fill");
    let out = String::from_utf8(out).expect("utf8");
    assert!(out.contains("<p>The intro</p>"), "{out}");
    assert!(!out.contains("draft intro"));
}
