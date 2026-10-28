use std::io::{Cursor, Write};

use systemprompt_files::documents::codecs::docx::{blank_docx, import_rich, materialise_rich};
use systemprompt_files::documents::model::layout::Align;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::docx_fixture::{entries, part};

const STYLES_XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Body"><w:name w:val="Body"/><w:uiPriority w:val="9"/>"#,
    r#"<w:pPr><w:textAlignment w:val="center"/><w:rPr><w:b/><w:kern w:val="4"/></w:rPr></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:eastAsia="SimSun"/><w:kern w:val="2"/></w:rPr>"#,
    r#"</w:style></w:styles>"#,
);

fn with_styles(styles: &str) -> Vec<u8> {
    let blank = entries(&blank_docx().expect("blank"));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content, ..) in &blank {
        let content = if name == "word/styles.xml" {
            styles.as_bytes()
        } else {
            content.as_slice()
        };
        writer.start_file(name.as_str(), options).expect("start");
        writer.write_all(content).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn restyled() -> String {
    let base = with_styles(STYLES_XML);
    let mut model = import_rich(&base).expect("import");
    let settings = model.document.as_mut().expect("settings");
    let body = settings
        .styles
        .iter_mut()
        .find(|style| style.id == "Body")
        .expect("Body style");
    body.color = Some("FF0000".to_owned());
    body.paragraph.align = Some(Align::Center);
    let out = materialise_rich(&base, &model).expect("materialise");
    let styles = part(&out, "word/styles.xml");
    let start = styles.find("w:styleId=\"Body\"").expect("Body");
    let end = styles[start..].find("</w:style>").expect("closed") + start;
    styles[start..end].to_owned()
}

fn before(style: &str, first: &str, second: &str) -> bool {
    let first = style.find(first).expect("first child");
    let second = style.find(second).expect("second child");
    first < second
}

#[test]
fn a_new_colour_goes_before_an_unmodelled_kern() {
    let style = restyled();
    assert!(style.contains("<w:color w:val=\"FF0000\"/>"));
    assert!(before(&style, "<w:color", "<w:kern w:val=\"2\"/>"));
    assert!(style.contains("w:eastAsia=\"SimSun\""), "rFonts merges attribute by attribute");
}

#[test]
fn a_new_jc_goes_before_an_unmodelled_text_alignment() {
    let style = restyled();
    assert!(before(&style, "<w:jc w:val=\"center\"/>", "<w:textAlignment"));
    assert!(style.contains("<w:uiPriority w:val=\"9\"/>"));
}

#[test]
fn the_run_properties_inside_a_style_paragraph_keep_their_bytes() {
    let style = restyled();
    assert!(style.contains("<w:rPr><w:b/><w:kern w:val=\"4\"/></w:rPr></w:pPr>"));
}
