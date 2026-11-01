use systemprompt_files::documents::codecs::docx::{blank_docx, import_rich, materialise_rich};
use systemprompt_files::documents::model::document::{
    DocumentSettings, NumberFormat, NumberingDef, NumberingLevel,
};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};

use super::docx_fixture::part;
use super::docx_styles_theme::{body, package, rels};

fn item(id: &str, level: u8, ordered: bool, numbering: Option<&str>, text: &str) -> Block {
    let mut kind = BlockKind::list_item(level, ordered);
    if let BlockKind::ListItem {
        numbering: slot, ..
    } = &mut kind
    {
        *slot = numbering.map(str::to_owned);
    }
    Block::new(id, kind, text)
}

fn bullets_def() -> NumberingDef {
    NumberingDef {
        id: "dots & dashes".to_owned(),
        levels: vec![
            NumberingLevel {
                format: NumberFormat::Bullet,
                text: String::new(),
                start: 1,
                indent: None,
            },
            NumberingLevel {
                format: NumberFormat::UpperRoman,
                text: String::new(),
                start: 4,
                indent: Some(50),
            },
        ],
    }
}

fn model() -> RichText {
    let mut model = RichText::of(vec![
        item("a", 0, false, None, "plain bullet"),
        item("b", 0, true, None, "plain number"),
        item("c", 0, false, Some("dots & dashes"), "custom"),
        item("d", 1, false, Some("dots & dashes"), "custom nested"),
        item("e", 0, true, None, "plain number again"),
    ]);
    model.document = Some(Box::new(DocumentSettings {
        numbering: vec![bullets_def()],
        ..DocumentSettings::default()
    }));
    model
}

#[test]
fn a_package_without_numbering_gains_the_part_and_its_wiring() {
    let base = blank_docx().expect("blank");
    let out = materialise_rich(&base, &model()).expect("materialise");
    let numbering = part(&out, "word/numbering.xml");
    assert!(numbering.starts_with("<?xml"));
    assert_eq!(
        numbering.matches("<w:abstractNum ").count(),
        3,
        "{numbering}"
    );
    assert_eq!(numbering.matches("<w:num ").count(), 3);
    assert!(numbering.contains(r#"<w:name w:val="dots &amp; dashes"/>"#));
    assert!(numbering.contains("<w:lvlText w:val=\"\u{2022}\"/>"));
    assert!(
        numbering.contains(
            r#"<w:start w:val="4"/><w:numFmt w:val="upperRoman"/><w:lvlText w:val="%2."/>"#
        )
    );
    assert!(numbering.contains(r#"<w:ind w:left="1000" w:hanging="360"/>"#));
    assert!(numbering.find("<w:abstractNum ") < numbering.find("<w:num "));
    assert!(part(&out, "[Content_Types].xml").contains("/word/numbering.xml"));
    assert!(part(&out, "word/_rels/document.xml.rels").contains("numbering.xml"));
}

#[test]
fn list_items_import_back_with_their_definitions() {
    let base = blank_docx().expect("blank");
    let out = materialise_rich(&base, &model()).expect("materialise");
    let again = import_rich(&out).expect("import");
    let texts: Vec<_> = again.blocks.iter().map(|b| b.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "plain bullet",
            "plain number",
            "custom",
            "custom nested",
            "plain number again"
        ]
    );
    assert!(matches!(
        &again.blocks[3].kind,
        BlockKind::ListItem { level: 1, ordered: true, numbering: Some(id), .. } if id == "dots & dashes"
    ));
    assert!(matches!(
        &again.blocks[4].kind,
        BlockKind::ListItem { ordered: true, .. }
    ));
    let defs = again.settings();
    let custom = defs.numbering("dots & dashes").expect("custom def");
    assert_eq!(custom.levels[1].start, 4);
    assert_eq!(custom.levels[1].format, NumberFormat::UpperRoman);
}

#[test]
fn existing_numbering_keeps_its_nums_and_gains_new_definitions_in_schema_order() {
    let held = concat!(
        r#"<?xml version="1.0"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:abstractNum w:abstractNumId="4"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum>"#,
        r#"<w:num w:numId="7"><w:abstractNumId w:val="4"/></w:num></w:numbering>"#,
    );
    let rels = rels(
        r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#,
    );
    let doc = body("<w:p><w:r><w:t>intro</w:t></w:r></w:p>");
    let base = package(&[
        ("word/document.xml", &doc),
        ("word/_rels/document.xml.rels", &rels),
        ("word/numbering.xml", held),
    ]);
    let out = materialise_rich(&base, &model()).expect("materialise");
    let numbering = part(&out, "word/numbering.xml");
    assert!(numbering.contains(r#"<w:num w:numId="7"><w:abstractNumId w:val="4"/></w:num>"#));
    assert!(
        numbering.contains(r#"<w:abstractNum w:abstractNumId="5">"#),
        "{numbering}"
    );
    assert!(numbering.contains(r#"<w:num w:numId="8">"#));
    let last_abstract = numbering.rfind("<w:abstractNum ").expect("abstract");
    let first_num = numbering.find("<w:num ").expect("num");
    assert!(last_abstract < first_num);
}

#[test]
fn an_unclosed_numbering_part_is_refused() {
    let rels = rels(
        r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#,
    );
    let doc = body("<w:p><w:r><w:t>intro</w:t></w:r></w:p>");
    let base = package(&[
        ("word/document.xml", &doc),
        ("word/_rels/document.xml.rels", &rels),
        ("word/numbering.xml", "<w:numbering>"),
    ]);
    assert!(materialise_rich(&base, &model()).is_err());
}
