use std::io::{Cursor, Write};

use systemprompt_files::documents::codecs::docx::{blank_docx, import_rich, materialise_rich};
use systemprompt_files::documents::model::RichText;
use systemprompt_files::documents::model::document::{StyleDef, StyleKind, Theme};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::docx_fixture::{entries, part};

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

const STYLES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:style w:type="paragraph" w:styleId="Brand"><w:name w:val="Brand Body"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/>"#,
    r#"<w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:rFonts w:ascii="Georgia"/><w:b/><w:i w:val="0"/><w:color w:val="1F4E79"/><w:sz w:val="23"/></w:rPr></w:style>"#,
    r#"<w:style w:type="character" w:styleId="Shout"><w:rPr><w:color w:val="auto"/><w:sz w:val="28"/></w:rPr></w:style>"#,
    r#"<w:style w:type="table" w:styleId="Grid"><w:name w:val="Grid"/></w:style>"#,
    r#"<w:style w:type="numbering" w:styleId="Skip"><w:name w:val="Skip"/></w:style>"#,
    r#"<w:style w:type="paragraph"><w:name w:val="NoId"/></w:style>"#,
    "</w:styles>",
);

const THEME: &str = concat!(
    r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements>"#,
    r#"<a:clrScheme name="x"><a:accent1><a:srgbClr val="112233"/></a:accent1><a:accent2><a:sysClr val="windowText"/></a:accent2><a:accent3><a:srgbClr val="445566"/></a:accent3></a:clrScheme>"#,
    r#"<a:fontScheme name="x"><a:majorFont><a:latin typeface="Cambria"/></a:majorFont><a:minorFont><a:latin typeface=""/></a:minorFont></a:fontScheme>"#,
    "</a:themeElements></a:theme>",
);

pub(super) fn package(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut held = entries(&blank_docx().expect("blank"));
    for (name, body) in parts {
        match held.iter_mut().find(|(entry, ..)| entry == name) {
            Some(slot) => slot.1 = body.as_bytes().to_vec(),
            None => held.push(((*name).to_owned(), body.as_bytes().to_vec(), 0, 0)),
        }
    }
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content, ..) in &held {
        writer.start_file(name.as_str(), options).expect("start");
        writer.write_all(content).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

pub(super) fn body(paragraphs: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document {W}><w:body>{paragraphs}</w:body></w:document>"#
    )
}

pub(super) fn rels(extra: &str) -> String {
    format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>{extra}</Relationships>"#
    )
}

fn styled() -> Vec<u8> {
    let rels = rels(
        r#"<Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#,
    );
    let doc =
        body(r#"<w:p><w:pPr><w:pStyle w:val="Brand"/></w:pPr><w:r><w:t>Hello</w:t></w:r></w:p>"#);
    package(&[
        ("word/document.xml", &doc),
        ("word/styles.xml", STYLES),
        ("word/_rels/document.xml.rels", &rels),
        ("word/theme/theme1.xml", THEME),
    ])
}

fn style<'a>(model: &'a RichText, id: &str) -> &'a StyleDef {
    model
        .document
        .as_ref()
        .and_then(|doc| doc.styles.iter().find(|s| s.id == id))
        .expect("style present")
}

#[test]
fn styles_read_their_kind_fonts_flags_colour_and_half_point_size() {
    let model = import_rich(&styled()).expect("import");
    let brand = style(&model, "Brand");
    assert_eq!(brand.name, "Brand Body");
    assert_eq!(brand.kind, StyleKind::Paragraph);
    assert_eq!(brand.based_on.as_deref(), Some("Normal"));
    assert_eq!(brand.next.as_deref(), Some("Normal"));
    assert_eq!(brand.font.as_deref(), Some("Georgia"));
    assert_eq!(brand.size.as_deref(), Some("11.5"));
    assert!(brand.bold);
    assert!(!brand.italic);
    assert_eq!(brand.color.as_deref(), Some("1F4E79"));
    let shout = style(&model, "Shout");
    assert_eq!(shout.kind, StyleKind::Character);
    assert_eq!(shout.color, None);
    assert_eq!(shout.size.as_deref(), Some("14"));
    assert_eq!(style(&model, "Grid").kind, StyleKind::Table);
    assert!(!model.settings().styles.iter().any(|s| s.id == "Skip"));
}

#[test]
fn the_theme_reads_its_fonts_and_only_rgb_accents() {
    let model = import_rich(&styled()).expect("import");
    let theme = model.settings().theme.expect("theme");
    assert_eq!(theme.heading_font.as_deref(), Some("Cambria"));
    assert_eq!(theme.body_font, None);
    assert_eq!(
        theme.palette,
        vec!["112233".to_owned(), "445566".to_owned()]
    );
}

#[test]
fn an_unreadable_styles_part_yields_no_styles() {
    let doc = body("<w:p><w:r><w:t>x</w:t></w:r></w:p>");
    let bytes = package(&[
        ("word/document.xml", &doc),
        ("word/styles.xml", "<w:styles><broken"),
    ]);
    let model = import_rich(&bytes).expect("import");
    assert!(model.settings().styles.is_empty());
    assert_eq!(model.blocks[0].text, "x");
}

#[test]
fn unchanged_styles_and_theme_keep_their_parts() {
    let bytes = styled();
    let model = import_rich(&bytes).expect("import");
    let out = materialise_rich(&bytes, &model).expect("materialise");
    assert_eq!(part(&out, "word/styles.xml"), STYLES);
    assert_eq!(part(&out, "word/theme/theme1.xml"), THEME);
}

#[test]
fn changed_and_new_styles_and_a_changed_theme_are_written() {
    let bytes = styled();
    let mut model = import_rich(&bytes).expect("import");
    let doc = model.document.as_mut().expect("settings");
    let brand = doc
        .styles
        .iter_mut()
        .find(|s| s.id == "Brand")
        .expect("brand");
    brand.italic = true;
    brand.size = Some("15".to_owned());
    doc.styles.push(StyleDef {
        id: "Quote".to_owned(),
        name: String::new(),
        kind: StyleKind::Character,
        based_on: Some("Brand".to_owned()),
        next: None,
        font: Some("Arial & Co".to_owned()),
        size: Some("9".to_owned()),
        bold: true,
        italic: false,
        color: Some("#AA0000".to_owned()),
        paragraph: Default::default(),
    });
    doc.theme = Some(Theme {
        heading_font: Some("Inter".to_owned()),
        body_font: Some("Lato".to_owned()),
        palette: vec!["#ABCDEF".to_owned(), "000000".to_owned()],
    });
    let out = materialise_rich(&bytes, &model).expect("materialise");
    let styles = part(&out, "word/styles.xml");
    assert!(
        styles.contains(r#"w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Brand"/>"#),
        "{styles}"
    );
    assert!(styles.contains("Arial &amp; Co"));
    assert!(styles.contains(r#"<w:sz w:val="18"/>"#));
    assert!(styles.contains(r#"<w:color w:val="AA0000"/>"#));
    let theme = part(&out, "word/theme/theme1.xml");
    assert!(theme.contains(r#"typeface="Inter""#), "{theme}");
    assert!(theme.contains(r#"typeface="Lato""#));
    assert!(theme.contains(r#"val="ABCDEF""#));
    let again = import_rich(&out).expect("reimport");
    let brand = style(&again, "Brand");
    assert!(brand.italic);
    assert_eq!(brand.size.as_deref(), Some("15"));
    assert_eq!(style(&again, "Quote").font.as_deref(), Some("Arial & Co"));
}
