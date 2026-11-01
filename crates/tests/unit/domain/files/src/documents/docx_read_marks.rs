use systemprompt_files::documents::codecs::docx::{import_rich, materialise_rich};
use systemprompt_files::documents::model::{Block, BlockKind, MarkKind};

use super::docx_styles_theme::{body, package, rels};

fn import(paragraphs: &str) -> Vec<Block> {
    let doc = body(paragraphs);
    let rels = rels(
        r#"<Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>"#,
    );
    let bytes = package(&[
        ("word/document.xml", &doc),
        ("word/_rels/document.xml.rels", &rels),
    ]);
    import_rich(&bytes).expect("import").blocks
}

fn kinds(block: &Block) -> Vec<(usize, usize, MarkKind)> {
    block
        .marks
        .iter()
        .map(|m| (m.start, m.end, m.kind.clone()))
        .collect()
}

#[test]
fn run_properties_become_marks() {
    let blocks = import(concat!(
        "<w:p>",
        r#"<w:r><w:rPr><w:b/><w:i/><w:u w:val="single"/><w:dstrike/><w:caps/></w:rPr><w:t>ab</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:rFonts w:ascii="Consolas"/></w:rPr><w:t>cd</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:rFonts w:ascii="Georgia"/><w:color w:val="FF0000"/><w:highlight w:val="yellow"/><w:sz w:val="21"/><w:vertAlign w:val="superscript"/></w:rPr><w:t>ef</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:color w:val="auto"/><w:shd w:fill="00FF00"/><w:vertAlign w:val="subscript"/></w:rPr><w:t>gh</w:t></w:r>"#,
        "</w:p>",
    ));
    let marks = kinds(&blocks[0]);
    assert_eq!(blocks[0].text, "abcdefgh");
    for kind in [
        MarkKind::Bold,
        MarkKind::Italic,
        MarkKind::Underline,
        MarkKind::Strike,
        MarkKind::Caps,
    ] {
        assert!(marks.contains(&(0, 2, kind)));
    }
    assert!(marks.contains(&(2, 4, MarkKind::Code)));
    assert!(marks.contains(&(
        4,
        6,
        MarkKind::Font {
            value: "Georgia".to_owned()
        }
    )));
    assert!(marks.contains(&(
        4,
        6,
        MarkKind::Color {
            value: "#FF0000".to_owned()
        }
    )));
    assert!(marks.contains(&(
        4,
        6,
        MarkKind::Highlight {
            value: "yellow".to_owned()
        }
    )));
    assert!(marks.contains(&(
        4,
        6,
        MarkKind::Size {
            value: "10.5".to_owned()
        }
    )));
    assert!(marks.contains(&(4, 6, MarkKind::Superscript)));
    assert!(marks.contains(&(
        6,
        8,
        MarkKind::Highlight {
            value: "#00FF00".to_owned()
        }
    )));
    assert!(marks.contains(&(6, 8, MarkKind::Subscript)));
    assert!(!marks.iter().any(|(s, _, k)| *s == 6 && k.key() == "color"));
}

#[test]
fn hyperlinks_resolve_relationships_and_anchors_without_their_look() {
    let blocks = import(concat!(
        r#"<w:p><w:hyperlink r:id="rId5"><w:r><w:rPr><w:u w:val="single"/><w:color w:val="0563C1"/></w:rPr><w:t>site</w:t></w:r></w:hyperlink>"#,
        r#"<w:hyperlink w:anchor="intro"><w:r><w:t>back</w:t></w:r></w:hyperlink></w:p>"#,
    ));
    let marks = kinds(&blocks[0]);
    assert!(marks.contains(&(
        0,
        4,
        MarkKind::Link {
            href: "https://example.com".to_owned()
        }
    )));
    assert!(marks.contains(&(
        4,
        8,
        MarkKind::Link {
            href: "#intro".to_owned()
        }
    )));
    assert_eq!(marks.len(), 2, "{marks:?}");
}

#[test]
fn simple_and_complex_fields_mark_their_result_runs() {
    let blocks = import(concat!(
        r#"<w:p><w:fldSimple w:instr=" page \* MERGEFORMAT"><w:r><w:t>3</w:t></w:r></w:fldSimple>"#,
        r#"<w:r><w:t xml:space="preserve"> of </w:t></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>NUMPAGES</w:instrText></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>9</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
        r#"<w:fldSimple w:instr="BOGUS"><w:r><w:t>!</w:t></w:r></w:fldSimple></w:p>"#,
    ));
    let marks = kinds(&blocks[0]);
    assert_eq!(blocks[0].text, "3 of 9!");
    assert!(marks.contains(&(
        0,
        1,
        MarkKind::Field {
            code: "PAGE".to_owned()
        }
    )));
    assert!(marks.contains(&(
        5,
        6,
        MarkKind::Field {
            code: "NUMPAGES".to_owned()
        }
    )));
    assert_eq!(marks.len(), 2, "{marks:?}");
}

#[test]
fn a_toc_field_makes_a_table_of_contents_block() {
    let blocks = import(concat!(
        r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve">TOC \o "1-2" \h</w:instrText></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Intro 1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
    ));
    assert!(
        blocks
            .iter()
            .any(|b| matches!(b.kind, BlockKind::Toc { levels: 2 })),
        "{blocks:?}"
    );
    assert!(blocks.iter().any(|b| b.text == "after"));
}

#[test]
fn tabs_breaks_and_containers_contribute_text() {
    let blocks = import(concat!(
        r#"<w:p><w:smartTag><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t>c</w:t></w:r></w:smartTag>"#,
        r#"<w:moveTo><w:r><w:cr/><w:t>d</w:t></w:r></w:moveTo></w:p>"#,
    ));
    assert_eq!(blocks[0].text, "a\tb\nc\nd");
}

#[test]
fn marked_runs_survive_a_materialise_round_trip() {
    let doc = body(concat!(
        r#"<w:p><w:r><w:rPr><w:b/><w:color w:val="1F4E79"/><w:sz w:val="28"/></w:rPr><w:t>Big</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:strike/><w:vertAlign w:val="superscript"/></w:rPr><w:t>up</w:t></w:r></w:p>"#,
    ));
    let bytes = package(&[("word/document.xml", &doc)]);
    let model = import_rich(&bytes).expect("import");
    let out = materialise_rich(&bytes, &model).expect("materialise");
    let again = import_rich(&out).expect("reimport");
    assert_eq!(kinds(&again.blocks[0]), kinds(&model.blocks[0]));
    assert!(kinds(&model.blocks[0]).contains(&(3, 5, MarkKind::Strike)));
}
