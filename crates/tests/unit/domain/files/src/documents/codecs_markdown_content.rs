use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::codecs::odf::{OdpCodec, OdtCodec};
use systemprompt_files::documents::codecs::pptx::PptxCodec;
use systemprompt_files::documents::codecs::xlsx::XlsxCodec;
use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec, TextCodec};
use systemprompt_provider_contracts::documents::{
    ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget, Projection,
};

use super::{SAMPLE_HTML, SAMPLE_MD, SAMPLE_TXT, apply_str, id};

const MARKDOWN: &str = "# Plan\n\nFirst paragraph with **bold**, *italic* and a \
                        [link](https://example.com).\n\nSecond paragraph.\n\n- one\n- two\n\n| A \
                        | B |\n|---|---|\n| 1 | 2 |\n";

fn insert_markdown(node: &str) -> EditOp {
    EditOp::InsertAfter {
        node: NodeTarget::Id(id(node)),
        kind: NodeKind::Paragraph,
        text: MARKDOWN.to_owned(),
        content_as: ContentAs::Markdown,
    }
}

fn append_markdown() -> EditOp {
    EditOp::AppendMarkdown {
        markdown: MARKDOWN.to_owned(),
    }
}

fn apply(codec: &dyn DocumentCodec, bytes: &[u8], ops: &[EditOp]) -> Vec<u8> {
    let base = codec.project(bytes).expect("project");
    codec.apply(bytes, &base, ops).expect("apply")
}

fn texts(projection: &Projection) -> Vec<String> {
    projection
        .nodes()
        .filter_map(|node| node.text.clone())
        .collect()
}

fn node_text(projection: &Projection, kind: NodeKind, text: &str) -> bool {
    projection
        .nodes()
        .any(|node| node.kind == kind && node.text.as_deref() == Some(text))
}

fn changed_entries(before: &[u8], after: &[u8]) -> Vec<String> {
    let before = super::docx_fixture::entries(before);
    let after = super::docx_fixture::entries(after);
    assert_eq!(
        before.iter().map(|e| &e.0).collect::<Vec<_>>(),
        after.iter().map(|e| &e.0).collect::<Vec<_>>()
    );
    before
        .iter()
        .zip(&after)
        .filter(|(b, a)| b.1 != a.1)
        .map(|(b, _)| b.0.clone())
        .collect()
}

#[test]
fn text_lowers_markdown_to_plain_lines() {
    let out = apply_str(&TextCodec, SAMPLE_TXT, &[insert_markdown("line[1]")]);
    let lines: Vec<&str> = out.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == "Plan")
        .expect("heading line");
    assert_eq!(
        &lines[at..at + 7],
        &[
            "Plan",
            "First paragraph with bold, italic and a link (https://example.com).",
            "Second paragraph.",
            "- one",
            "- two",
            "A | B",
            "1 | 2",
        ]
    );
    assert_eq!(at, 1);
}

#[test]
fn markdown_passes_markdown_through_and_outlines_it() {
    let out = apply_str(&MarkdownCodec, SAMPLE_MD, &[append_markdown()]);
    assert!(out.contains("First paragraph with **bold**, *italic*"));
    assert!(out.contains("<script>alert(1)</script>\n\n# Plan"));
    let projection = MarkdownCodec.project(out.as_bytes()).expect("project");
    assert!(node_text(
        &projection,
        NodeKind::Heading { level: 1 },
        "Plan"
    ));
    assert!(node_text(&projection, NodeKind::ListItem, "two"));
}

#[test]
fn html_writes_semantic_elements() {
    let out = apply_str(&HtmlCodec, SAMPLE_HTML, &[append_markdown()]);
    for expected in [
        "<h1>Plan</h1>",
        "<strong>bold</strong>",
        "<em>italic</em>",
        "<a href=\"https://example.com\"",
        "<li>one</li>",
        "<th>A</th>",
        "<td>2</td>",
    ] {
        assert!(out.contains(expected), "missing {expected} in {out}");
    }
    assert!(out.contains("<title>Keep Me</title>"));
    let projection = HtmlCodec.project(out.as_bytes()).expect("project");
    assert!(node_text(
        &projection,
        NodeKind::Heading { level: 1 },
        "Plan"
    ));
}

#[test]
fn docx_writes_styled_paragraphs_runs_and_a_table() {
    let before = super::docx_fixture::fixture();
    let after = apply(&DocxCodec, &before, &[insert_markdown("pid:2B3C4D5E")]);
    assert_eq!(changed_entries(&before, &after), vec!["word/document.xml"]);
    let xml = super::docx_fixture::part(&after, "word/document.xml");
    assert!(xml.contains("<w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>Plan</w:t>"));
    assert!(xml.contains("<w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r>"));
    assert!(xml.contains("<w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r>"));
    assert!(xml.contains("<w:t xml:space=\"preserve\"> (https://example.com)</w:t>"));
    assert_eq!(xml.matches("<w:tbl>").count(), 2);
    let plan = xml.find(">Plan<").expect("plan");
    assert!(xml.find("strongly.").expect("anchor") < plan);
    assert!(plan < xml.find("First point").expect("next paragraph"));
    let projection = DocxCodec.project(&after).expect("project");
    assert!(node_text(
        &projection,
        NodeKind::Heading { level: 1 },
        "Plan"
    ));
    let all = texts(&projection);
    for expected in ["- one", "- two", "A", "B", "1", "2", "Second paragraph."] {
        assert!(all.iter().any(|t| t == expected), "missing {expected}");
    }
}

#[test]
fn docx_append_lands_before_the_section_properties() {
    let after = apply(
        &DocxCodec,
        &super::docx_fixture::fixture(),
        &[append_markdown()],
    );
    let xml = super::docx_fixture::part(&after, "word/document.xml");
    let plan = xml.find(">Plan<").expect("plan");
    assert!(xml.find("Ops &amp; Finance").expect("last paragraph") < plan);
    assert!(plan < xml.find("<w:sectPr>").expect("section"));
}

#[test]
fn pptx_writes_bold_headings_and_bullets_in_the_text_frame() {
    let before = super::pptx_fixture::two_slide_deck();
    let after = apply(
        &PptxCodec,
        &before,
        &[insert_markdown("slide[2]/shape[1]/p[0]")],
    );
    assert_eq!(
        changed_entries(&before, &after),
        vec!["ppt/slides/slide1.xml"]
    );
    let xml = super::pptx_fixture::part(&after, "ppt/slides/slide1.xml");
    assert!(xml.contains("<a:rPr lang=\"en-US\" b=\"1\" dirty=\"0\"></a:rPr><a:t>Plan</a:t>"));
    assert!(xml.contains("<a:rPr lang=\"en-US\" i=\"1\" dirty=\"0\"></a:rPr><a:t>italic</a:t>"));
    assert_eq!(xml.matches("<a:buChar char=\"\u{2022}\"/>").count(), 2);
    let projection = PptxCodec.project(&after).expect("project");
    let all = texts(&projection);
    for expected in ["Plan", "one", "A | B", "1 | 2", "Second paragraph."] {
        assert!(all.iter().any(|t| t == expected), "missing {expected}");
    }
}

#[test]
fn xlsx_writes_table_rows_as_cells_and_other_blocks_as_single_cells() {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.cell_mut("A1").set_value_string("Name");
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).expect("write");
    let after = apply(&XlsxCodec, &bytes, &[append_markdown()]);
    let projection = XlsxCodec.project(&after).expect("project");
    let cell = |at: &str| {
        projection
            .find(&id(&format!("sheet[Sheet1]/{at}")))
            .and_then(|node| node.text.clone())
    };
    assert_eq!(cell("A1").as_deref(), Some("Name"));
    assert_eq!(cell("A2").as_deref(), Some("Plan"));
    assert_eq!(cell("A5").as_deref(), Some("- one"));
    assert_eq!(cell("A7").as_deref(), Some("A"));
    assert_eq!(cell("B7").as_deref(), Some("B"));
    assert_eq!(cell("B8").as_deref(), Some("2"));
    assert!(cell("B2").is_none());
}

const ODT_BODY: &str = concat!(
    "<office:text><text:sequence-decls/>",
    "<text:h text:outline-level=\"1\">Report</text:h>",
    "<text:p>Revenue grew</text:p>",
    "<text:p>Closing</text:p></office:text>",
);

#[test]
fn odt_writes_headings_lists_links_and_a_table() {
    let before = super::odf_fixture::package("application/vnd.oasis.opendocument.text", ODT_BODY);
    let after = apply(&OdtCodec, &before, &[insert_markdown("body/p[1]")]);
    super::odf_fixture::assert_only_content_changed(&before, &after);
    let xml = super::docx_fixture::part(&after, "content.xml");
    assert!(
        xml.contains("<text:p>Revenue grew</text:p><text:h text:outline-level=\"1\">Plan</text:h>")
    );
    assert!(xml.contains(
        "<text:a xlink:type=\"simple\" xlink:href=\"https://example.com\">link</text:a>"
    ));
    assert!(xml.contains(
        "<text:list><text:list-item><text:p>one</text:p></text:list-item><text:list-item><text:p>two</text:p></text:list-item></text:list>"
    ));
    assert!(xml.contains("<table:table-column table:number-columns-repeated=\"2\"/>"));
    let projection = OdtCodec.project(&after).expect("project");
    assert!(node_text(
        &projection,
        NodeKind::Heading { level: 1 },
        "Plan"
    ));
    assert!(node_text(&projection, NodeKind::ListItem, "one"));
    assert!(texts(&projection).iter().any(|t| t == "Closing"));
}

#[test]
fn odt_append_lands_at_the_end_of_the_body() {
    let before = super::odf_fixture::package("application/vnd.oasis.opendocument.text", ODT_BODY);
    let after = apply(&OdtCodec, &before, &[append_markdown()]);
    let xml = super::docx_fixture::part(&after, "content.xml");
    assert!(xml.contains("<text:p>Closing</text:p><text:h text:outline-level=\"1\">Plan</text:h>"));
}

#[test]
fn odp_writes_paragraphs_and_table_rows_in_the_frame() {
    let body = concat!(
        "<office:presentation><draw:page draw:name=\"one\" draw:master-page-name=\"Default\">",
        "<draw:frame presentation:class=\"outline\"><draw:text-box><text:p>Plain B</text:p>",
        "</draw:text-box></draw:frame></draw:page></office:presentation>",
    );
    let before =
        super::odf_fixture::package("application/vnd.oasis.opendocument.presentation", body);
    let after = apply(
        &OdpCodec,
        &before,
        &[insert_markdown("slide[1]/shape[0]/p[0]")],
    );
    super::odf_fixture::assert_only_content_changed(&before, &after);
    let xml = super::docx_fixture::part(&after, "content.xml");
    assert!(xml.contains("<text:p>Plain B</text:p><text:p>Plan</text:p>"));
    assert!(!xml.contains("<table:table"));
    let projection = OdpCodec.project(&after).expect("project");
    let all = texts(&projection);
    for expected in ["Plan", "A | B", "1 | 2", "one"] {
        assert!(all.iter().any(|t| t == expected), "missing {expected}");
    }
}
