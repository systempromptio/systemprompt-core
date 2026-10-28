use systemprompt_files::documents::codecs::pptx::PptxCodec;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget,
};

use super::pptx_fixture::{apply, node, part, project, two_slide_deck};

const MARKDOWN: &str = "# Title\n\nPlain **bold** *it* `code` [site](https://example.com)\n\n- bullet\n  - nested\n\n1. first\n2. second\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```\nlet x;\nlet y;\n```\n\n<!-- pending: add numbers -->\n";

fn markdown_after(id: &str, markdown: &str) -> EditOp {
    EditOp::InsertAfter {
        node: NodeTarget::Id(node(id)),
        kind: NodeKind::Paragraph,
        text: markdown.to_owned(),
        content_as: ContentAs::Markdown,
    }
}

#[test]
fn markdown_insert_lowers_every_block_kind() {
    let deck = two_slide_deck();
    let out = apply(&deck, &[markdown_after("slide[2]/shape[1]/p[0]", MARKDOWN)]);
    let xml = part(&out, "ppt/slides/slide1.xml");
    assert!(
        xml.contains("b=\"1\" dirty=\"0\"><a:t>Title</a:t>") || xml.contains("<a:t>Title</a:t>")
    );
    assert!(xml.contains("<a:t>bold</a:t>"));
    assert!(xml.contains("u=\"sng\""));
    assert!(xml.contains(" (https://example.com)"));
    assert!(xml.contains("typeface=\"Courier New\""));
    assert!(xml.contains("<a:buChar char=\"\u{2022}\"/>"));
    assert!(xml.contains("lvl=\"1\""));
    assert!(xml.contains("<a:buAutoNum type=\"arabicPeriod\" startAt=\"1\"/>"));
    assert!(xml.contains("<a:t>1 | 2</a:t>"));
    assert!(xml.contains("<a:t>let x;</a:t></a:r><a:br/>"));
    assert!(xml.contains("add numbers"));
    assert!(xml.contains("<a:buNone/>"));
    let reprojected = project(&out);
    let shape = reprojected.find(&node("slide[2]/shape[1]")).expect("shape");
    assert!(shape.children.len() > 8, "{}", shape.children.len());
}

#[test]
fn append_markdown_goes_after_the_last_text_paragraph() {
    let deck = two_slide_deck();
    let out = apply(
        &deck,
        &[EditOp::AppendMarkdown {
            markdown: "closing words".to_owned(),
        }],
    );
    let xml = part(&out, "ppt/slides/slide1.xml");
    let intro = xml.find("hello {{name}}").expect("note");
    let closing = xml.find("<a:t>closing words</a:t>").expect("closing");
    assert!(intro < closing);
}

#[test]
fn slide_targets_and_odd_paths_are_refused() {
    let deck = two_slide_deck();
    let base = project(&deck);
    for (id, unknown) in [
        ("slide[2]", false),
        ("slide[2]/shape[1]", false),
        ("slide[9]/shape[0]/p[0]", true),
        ("slide[2]/shape[1]/p[99]", true),
        ("slide[0]/shape[0]/p[0]", true),
    ] {
        let err = PptxCodec
            .apply(&deck, &base, &[markdown_after(id, "x")])
            .expect_err(id);
        if unknown {
            assert!(
                matches!(err, CodecError::UnknownNode { .. }),
                "{id}: {err:?}"
            );
        } else {
            assert!(
                matches!(err, CodecError::UnsupportedOp { .. }),
                "{id}: {err:?}"
            );
        }
    }
}

#[test]
fn multi_line_replace_carries_run_properties_into_breaks() {
    let deck = two_slide_deck();
    let base = project(&deck);
    let id = "slide[2]/shape[0]/p[0]";
    let out = apply(
        &deck,
        &[EditOp::ReplaceText {
            node: NodeTarget::Id(node(id)),
            expected_hash: Some(base.find(&node(id)).expect("title").hash8.clone()),
            text: "Line one\nLine two".to_owned(),
            content_as: ContentAs::Text,
        }],
    );
    let xml = part(&out, "ppt/slides/slide1.xml");
    assert!(
        xml.contains("<a:br><a:rPr lang=\"en-US\" b=\"1\"/></a:br>"),
        "{xml}"
    );
}

#[test]
fn insert_after_a_paragraph_with_empty_ppr_adds_bu_none() {
    let deck = two_slide_deck();
    let out = apply(
        &deck,
        &[EditOp::InsertAfter {
            node: NodeTarget::Id(node("slide[2]/shape[1]/p[1]")),
            kind: NodeKind::Paragraph,
            text: "after hiring".to_owned(),
            content_as: ContentAs::Text,
        }],
    );
    let xml = part(&out, "ppt/slides/slide1.xml");
    assert!(
        xml.contains("<a:pPr lvl=\"1\"><a:buNone/></a:pPr>"),
        "{xml}"
    );
}
