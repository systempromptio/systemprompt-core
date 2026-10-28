use chrono::{TimeZone, Utc};
use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::codecs::odf::OdtCodec;
use systemprompt_files::documents::codecs::pptx::PptxCodec;
use systemprompt_files::documents::codecs::xlsx::XlsxCodec;
use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec, TextCodec};
use systemprompt_files::documents::pdf::{PdfError, RenderRequest, pdf_file_name, render};
use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_provider_contracts::documents::DocumentCodec;
use systemprompt_test_fixtures::{ParsedPdf, parse_pdf};

use super::docx_fixture::{DOCUMENT_XML, package_with};
use super::odf_fixture::package;
use super::pptx_fixture::two_slide_deck;

const MARKDOWN: &str = "# Launch plan\n\nThe launch is in March.\n\n- first task\n  - nested task\n- second task\n\n| Region | Total |\n|---|---|\n| North | 12 |\n";

const HTML: &str = "<html><body><h1>Launch plan</h1><p>The launch is in March.</p><ul><li>first task<ul><li>nested task</li></ul></li></ul><table><tr><td>Region</td><td>Total</td></tr><tr><td>North</td><td>12</td></tr></table><img src=\"https://example.test/x.png\" alt=\"logo\"></body></html>";

const ODT_MIME: &str = "application/vnd.oasis.opendocument.text";

const ODT_BODY: &str = concat!(
    "<office:text>",
    r#"<text:h text:outline-level="1">Launch plan</text:h>"#,
    "<text:p>The launch is in March.</text:p>",
    "<text:list><text:list-item><text:p>first task</text:p></text:list-item></text:list>",
    r#"<table:table table:name="T"><table:table-column/><table:table-row>"#,
    "<table:table-cell><text:p>Region</text:p></table:table-cell>",
    "<table:table-cell><text:p>North</text:p></table:table-cell></table:table-row></table:table>",
    "</office:text>",
);

fn revision() -> DocumentRevisionId {
    DocumentRevisionId::new("drev_01HZZZZZZZZZZZZZZZZZZZZZZZ")
}

fn render_with(
    codec: &dyn DocumentCodec,
    bytes: &[u8],
    name: &str,
    max_pages: usize,
) -> Result<Vec<u8>, PdfError> {
    let projection = codec.project(bytes).expect("project");
    let revision = revision();
    render(
        &projection,
        &RenderRequest {
            name,
            revision: &revision,
            created_at: Utc
                .with_ymd_and_hms(2026, 10, 27, 9, 30, 0)
                .single()
                .expect("date"),
            max_pages,
        },
    )
}

fn pdf_of(codec: &dyn DocumentCodec, bytes: &[u8], name: &str) -> Vec<u8> {
    render_with(codec, bytes, name, 500).expect("render")
}

fn text_of(pdf: &[u8]) -> String {
    parse_pdf(pdf).text()
}

fn info_of(pdf: &[u8], key: &str) -> String {
    parse_pdf(pdf).info(key).expect("info key")
}

fn assert_carries(text: &str, needles: &[&str]) {
    for needle in needles {
        assert!(text.contains(needle), "{needle:?} missing from:\n{text}");
    }
}

#[test]
fn markdown_renders_headings_paragraphs_list_items_and_table_cells() {
    let pdf = pdf_of(&MarkdownCodec, MARKDOWN.as_bytes(), "plan.md");
    assert!(pdf.starts_with(b"%PDF-"));
    let text = text_of(&pdf);
    assert_carries(
        &text,
        &[
            "Launch plan",
            "The launch is in March.",
            "first task",
            "nested task",
            "Page 1 of 1",
        ],
    );
    assert_eq!(info_of(&pdf, "Title"), "plan.md");
    assert_eq!(info_of(&pdf, "Producer"), "systemprompt");
    assert_eq!(info_of(&pdf, "Subject"), format!("revision {}", revision()));
    assert!(info_of(&pdf, "CreationDate").contains("20261027093000"));
}

#[test]
fn html_renders_the_tree_and_omits_the_image() {
    let pdf = pdf_of(&HtmlCodec, HTML.as_bytes(), "plan.html");
    let text = text_of(&pdf);
    assert_carries(
        &text,
        &[
            "Launch plan",
            "The launch is in March.",
            "first task",
            "nested task",
            "Region",
            "North",
            "12",
        ],
    );
    assert!(!text.contains("example.test") && !text.contains("logo"));
}

#[test]
fn docx_renders_headings_paragraphs_list_items_and_table_cells() {
    let pdf = pdf_of(&DocxCodec, &package_with(DOCUMENT_XML), "report.docx");
    let text = text_of(&pdf);
    assert_carries(
        &text,
        &[
            "Quarterly Report",
            "Revenue grew strongly.",
            "First point",
            "Region",
            "Total",
        ],
    );
}

#[test]
fn xlsx_renders_one_section_per_sheet_with_a_lettered_grid() {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.cell_mut("A1").set_value_string("Name");
    sheet.cell_mut("C1").set_value_string("Qty");
    sheet.cell_mut("A2").set_value_string("apples");
    sheet.cell_mut("C2").set_value_number(3);
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).expect("write");
    let pdf = pdf_of(&XlsxCodec, &bytes, "stock.xlsx");
    let text = text_of(&pdf);
    assert_carries(
        &text,
        &["Sheet1", "A", "B", "C", "Name", "Qty", "apples", "3"],
    );
}

#[test]
fn pptx_renders_one_page_per_slide_with_title_and_body() {
    let parsed: ParsedPdf = parse_pdf(&pdf_of(&PptxCodec, &two_slide_deck(), "deck.pptx"));
    assert_eq!(parsed.pages.len(), 2);
    assert_carries(&parsed.pages[0], &["Intro", "Page 1 of 2"]);
    assert_carries(
        &parsed.pages[1],
        &[
            "Agenda",
            "Revenue & costs",
            "Hiring plan",
            "Q1",
            "10",
            "Page 2 of 2",
        ],
    );
}

#[test]
fn odt_renders_headings_paragraphs_list_items_and_table_cells() {
    let pdf = pdf_of(&OdtCodec, &package(ODT_MIME, ODT_BODY), "plan.odt");
    let text = text_of(&pdf);
    assert_carries(
        &text,
        &[
            "Launch plan",
            "The launch is in March.",
            "first task",
            "Region",
            "North",
        ],
    );
}

#[test]
fn text_renders_each_line_in_monospace() {
    let pdf = pdf_of(&TextCodec, b"fn main() {}\n    indented\n", "main.txt");
    let text = text_of(&pdf);
    assert_carries(&text, &["fn main() {}", "indented"]);
    assert!(parse_pdf(&pdf).has_font("Courier"));
}

#[test]
fn the_page_cap_refuses_a_longer_rendering() {
    let long: String = (0..400)
        .map(|n| format!("Paragraph {n} of the long document.\n\n"))
        .collect();
    let refused =
        render_with(&MarkdownCodec, long.as_bytes(), "long.md", 2).expect_err("over the cap");
    assert!(matches!(refused, PdfError::PageCap(cap) if cap.max_pages == 2));
    let allowed =
        render_with(&MarkdownCodec, long.as_bytes(), "long.md", 500).expect("under the cap");
    assert!(parse_pdf(&allowed).pages.len() > 2);
}

#[test]
fn the_same_revision_renders_to_the_same_bytes() {
    let first = pdf_of(&MarkdownCodec, MARKDOWN.as_bytes(), "plan.md");
    let second = pdf_of(&MarkdownCodec, MARKDOWN.as_bytes(), "plan.md");
    assert_eq!(first, second);
}

#[test]
fn a_character_outside_win_ansi_is_shown_as_a_question_mark() {
    let pdf = pdf_of(&TextCodec, "caf\u{e9} \u{65e5}\u{672c}".as_bytes(), "x.txt");
    let text = text_of(&pdf);
    assert_carries(&text, &["caf\u{e9} ??"]);
}

#[test]
fn the_pdf_file_name_swaps_the_extension() {
    assert_eq!(pdf_file_name("report.docx"), "report.pdf");
    assert_eq!(pdf_file_name("notes"), "notes.pdf");
    assert_eq!(pdf_file_name(".hidden"), ".hidden.pdf");
    assert_eq!(pdf_file_name("v1.2 draft"), "v1.2 draft.pdf");
}
