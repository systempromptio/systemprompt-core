use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::codecs::odf::{OdtCodec, blank_odt};
use systemprompt_files::documents::codecs::pptx::PptxCodec;
use systemprompt_files::documents::codecs::xlsx::XlsxCodec;
use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec, TextCodec};
use systemprompt_provider_contracts::documents::{
    ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
};

use super::{SAMPLE_HTML, SAMPLE_MD, SAMPLE_TXT};

const INTENT: &str = "Goals: the three outcomes this quarter commits to";
const FILLED: &str = "Ship the shared document store";

fn xlsx() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.cell_mut("A1").set_value_string("Name");
    sheet.cell_mut("A2").set_value_string("apples");
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut out).expect("write");
    out
}

fn anchor(projection: &Projection, kinds: &[NodeKind]) -> ProjectionNode {
    projection
        .nodes()
        .find(|node| {
            kinds.contains(&node.kind) && node.text.as_deref().is_none_or(|t| !t.is_empty())
        })
        .cloned()
        .expect("anchor node")
}

fn pending(projection: &Projection) -> Option<&ProjectionNode> {
    projection
        .nodes()
        .find(|node| node.kind == NodeKind::Pending)
}

fn round_trip(codec: &dyn DocumentCodec, bytes: &[u8], kinds: &[NodeKind]) {
    let base = codec.project(bytes).expect("project");
    let at = anchor(&base, kinds);
    let insert = EditOp::InsertAfter {
        node: NodeTarget::Id(at.id.clone()),
        kind: NodeKind::Pending,
        text: INTENT.to_owned(),
        content_as: ContentAs::Text,
    };
    let laid_out = codec
        .apply(bytes, &base, &[insert])
        .expect("insert pending");
    let skeleton = codec.project(&laid_out).expect("reproject");
    let placeholder = pending(&skeleton)
        .unwrap_or_else(|| panic!("{:?} projects no pending node", codec.formats()))
        .clone();
    assert_eq!(placeholder.text.as_deref(), Some(INTENT));

    let fill = EditOp::ReplaceText {
        node: NodeTarget::Id(placeholder.id.clone()),
        expected_hash: Some(placeholder.hash8.clone()),
        text: FILLED.to_owned(),
        content_as: ContentAs::Text,
    };
    let filled = codec.apply(&laid_out, &skeleton, &[fill]).expect("fill");
    let done = codec.project(&filled).expect("project filled");
    assert!(
        pending(&done).is_none(),
        "{:?} keeps a placeholder",
        codec.formats()
    );
    assert!(
        done.nodes()
            .any(|node| node.text.as_deref() == Some(FILLED)),
        "{:?} lacks the filled text",
        codec.formats()
    );
}

#[test]
fn text_pending_round_trips() {
    round_trip(&TextCodec, SAMPLE_TXT, &[NodeKind::Text]);
    let laid = super::apply_str(
        &TextCodec,
        SAMPLE_TXT,
        &[super::insert_after("line[1]", NodeKind::Pending, INTENT)],
    );
    assert!(laid.contains(&format!("[pending: {INTENT}]")), "{laid}");
}

#[test]
fn markdown_pending_is_a_comment_with_the_intent_in_italics() {
    round_trip(&MarkdownCodec, SAMPLE_MD, &[NodeKind::Text]);
    let laid = super::apply_str(
        &MarkdownCodec,
        SAMPLE_MD,
        &[super::insert_after("line[1]", NodeKind::Pending, INTENT)],
    );
    assert!(
        laid.contains(&format!("<!-- pending: {INTENT} --> *{INTENT}*")),
        "{laid}"
    );
}

#[test]
fn markdown_content_lays_out_a_skeleton() {
    let markdown = format!("# Plan\n\n<!-- pending: {INTENT} -->\n\nclosing words\n");
    let laid = super::apply_str(
        &TextCodec,
        SAMPLE_TXT,
        &[EditOp::AppendMarkdown { markdown }],
    );
    let projection = TextCodec.project(laid.as_bytes()).expect("project");
    assert_eq!(
        pending(&projection).and_then(|node| node.text.as_deref()),
        Some(INTENT)
    );
}

#[test]
fn html_pending_carries_data_pending() {
    round_trip(&HtmlCodec, SAMPLE_HTML, &[NodeKind::Paragraph]);
}

#[test]
fn docx_pending_round_trips() {
    round_trip(
        &DocxCodec,
        &super::docx_fixture::fixture(),
        &[NodeKind::Paragraph],
    );
}

#[test]
fn pptx_pending_round_trips() {
    round_trip(
        &PptxCodec,
        &super::pptx_fixture::two_slide_deck(),
        &[NodeKind::Paragraph],
    );
}

#[test]
fn xlsx_pending_round_trips() {
    round_trip(&XlsxCodec, &xlsx(), &[NodeKind::Row]);
}

#[test]
fn odt_pending_round_trips() {
    let body = "<office:text><text:p>Opening words</text:p></office:text>";
    let bytes = super::odf_fixture::package("application/vnd.oasis.opendocument.text", body);
    round_trip(&OdtCodec, &bytes, &[NodeKind::Paragraph]);
    blank_odt().expect("blank odt");
}
