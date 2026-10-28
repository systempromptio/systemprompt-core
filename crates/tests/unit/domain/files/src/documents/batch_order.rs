use systemprompt_files::documents::codecs::HtmlCodec;
use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::codecs::pptx::PptxCodec;
use systemprompt_files::documents::codecs::xlsx::XlsxCodec;
use systemprompt_provider_contracts::documents::{
    ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget, Projection,
};

use super::{SAMPLE_HTML, hash_of, id, insert_after, replace};

fn texts(projection: &Projection) -> Vec<String> {
    projection
        .nodes()
        .filter_map(|node| node.text.clone())
        .collect()
}

fn run(codec: &dyn DocumentCodec, bytes: &[u8], ops: &[EditOp]) -> Vec<String> {
    let base = codec.project(bytes).expect("project");
    let out = codec.apply(bytes, &base, ops).expect("apply");
    texts(&codec.project(&out).expect("reproject"))
}

fn follows(texts: &[String], run: &[&str]) {
    let start = texts
        .iter()
        .position(|text| text == run[0])
        .unwrap_or_else(|| panic!("{} missing from {texts:?}", run[0]));
    let found: Vec<&str> = texts[start..]
        .iter()
        .take(run.len())
        .map(String::as_str)
        .collect();
    assert_eq!(found, run, "{texts:?}");
}

#[test]
fn docx_inserts_on_one_anchor_land_in_op_order() {
    let bytes = super::docx_fixture::fixture();
    let base = DocxCodec.project(&bytes).expect("project");
    let texts = run(
        &DocxCodec,
        &bytes,
        &[
            insert_after("pid:1A2B3C4D", NodeKind::Paragraph, "one"),
            insert_after("pid:1A2B3C4D", NodeKind::Paragraph, "two"),
            replace("pid:2B3C4D5E", hash_of(&base, "pid:2B3C4D5E"), "after"),
        ],
    );
    follows(&texts, &["Quarterly Report", "one", "two", "after"]);
}

#[test]
fn html_inserts_on_one_anchor_land_in_op_order() {
    let texts = run(
        &HtmlCodec,
        SAMPLE_HTML,
        &[
            insert_after("body/h1[1]", NodeKind::Paragraph, "one"),
            insert_after("body/h1[1]", NodeKind::Paragraph, "two"),
        ],
    );
    follows(&texts, &["Heading", "one", "two", "First bold para"]);
}

#[test]
fn pptx_targets_resolve_against_the_deck_as_read() {
    let deck = super::pptx_fixture::two_slide_deck();
    let base = PptxCodec.project(&deck).expect("project");
    let texts = run(
        &PptxCodec,
        &deck,
        &[
            insert_after("slide[2]/shape[1]/p[0]", NodeKind::Paragraph, "one"),
            insert_after("slide[2]/shape[1]/p[0]", NodeKind::Paragraph, "two"),
            replace(
                "slide[2]/shape[1]/p[1]",
                hash_of(&base, "slide[2]/shape[1]/p[1]"),
                "after",
            ),
        ],
    );
    let anchor = base
        .find(&id("slide[2]/shape[1]/p[0]"))
        .and_then(|node| node.text.clone())
        .expect("anchor");
    follows(&texts, &[anchor.as_str(), "one", "two", "after"]);
}

#[test]
fn pptx_a_removed_slide_does_not_shift_later_targets() {
    let deck = super::pptx_fixture::two_slide_deck();
    let base = PptxCodec.project(&deck).expect("project");
    let texts = run(
        &PptxCodec,
        &deck,
        &[
            EditOp::Delete {
                node: NodeTarget::Id(id("slide[1]")),
                expected_hash: Some(hash_of(&base, "slide[1]")),
            },
            replace(
                "slide[2]/shape[1]/p[1]",
                hash_of(&base, "slide[2]/shape[1]/p[1]"),
                "kept",
            ),
        ],
    );
    assert!(texts.iter().any(|text| text == "kept"), "{texts:?}");
}

#[test]
fn xlsx_targets_resolve_against_the_workbook_as_read() {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.cell_mut("A1").set_value_string("head");
    sheet.cell_mut("A2").set_value_string("apples");
    sheet.cell_mut("A3").set_value_string("pears");
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).expect("write");
    let base = XlsxCodec.project(&bytes).expect("project");
    let out = XlsxCodec
        .apply(
            &bytes,
            &base,
            &[
                insert_after("sheet[Sheet1]/row[2]", NodeKind::Row, "one"),
                insert_after("sheet[Sheet1]/row[2]", NodeKind::Row, "two"),
                replace(
                    "sheet[Sheet1]/row[3]",
                    hash_of(&base, "sheet[Sheet1]/row[3]"),
                    "plums",
                ),
            ],
        )
        .expect("apply");
    let after = XlsxCodec.project(&out).expect("reproject");
    let row = |n: u32| {
        after
            .find(&id(&format!("sheet[Sheet1]/row[{n}]")))
            .and_then(|node| node.text.clone())
    };
    assert_eq!(row(2).as_deref(), Some("apples"));
    assert_eq!(row(3).as_deref(), Some("one"));
    assert_eq!(row(4).as_deref(), Some("two"));
    assert_eq!(row(5).as_deref(), Some("plums"));
}

#[test]
fn every_codec_names_its_insert_kinds() {
    use systemprompt_files::documents::codecs::CodecRegistry;
    use systemprompt_provider_contracts::documents::DocumentFormat;
    let codecs = CodecRegistry::discover();
    let kinds = |format| {
        codecs
            .for_format(format)
            .expect("codec")
            .insert_kinds(format)
    };
    assert_eq!(
        kinds(DocumentFormat::Docx),
        &["heading", "paragraph", "list_item", "pending"]
    );
    assert_eq!(
        kinds(DocumentFormat::Pptx),
        &["paragraph", "list_item", "pending"]
    );
    assert_eq!(kinds(DocumentFormat::Xlsx), &["row", "pending"]);
    assert!(kinds(DocumentFormat::Ods).is_empty());
}

#[test]
fn pptx_a_markdown_insert_shifts_later_targets_by_every_paragraph() {
    let deck = super::pptx_fixture::two_slide_deck();
    let base = PptxCodec.project(&deck).expect("project");
    let texts = run(
        &PptxCodec,
        &deck,
        &[
            EditOp::InsertAfter {
                node: NodeTarget::Id(id("slide[2]/shape[1]/p[0]")),
                kind: NodeKind::Paragraph,
                text: "first\n\nsecond".to_owned(),
                content_as: ContentAs::Markdown,
            },
            insert_after("slide[2]/shape[1]/p[0]", NodeKind::Paragraph, "third"),
            replace(
                "slide[2]/shape[1]/p[1]",
                hash_of(&base, "slide[2]/shape[1]/p[1]"),
                "after",
            ),
        ],
    );
    let anchor = base
        .find(&id("slide[2]/shape[1]/p[0]"))
        .and_then(|node| node.text.clone())
        .expect("anchor");
    follows(
        &texts,
        &[anchor.as_str(), "first", "second", "third", "after"],
    );
}
