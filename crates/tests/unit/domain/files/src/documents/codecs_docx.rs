use systemprompt_files::documents::codecs::docx::{DocxCodec, blank_docx};
use systemprompt_provider_contracts::documents::{
    DocumentCodec, DocumentFormat, NodeKind, TemplateParams,
};

use super::docx_fixture::{
    DOCUMENT_XML, IMAGE_BYTES, entries, fixture, package_with, part, project, replace, text_of,
};

#[test]
fn replace_text_rewrites_only_the_document_part() {
    let bytes = fixture();
    let edited = replace(&bytes, "pid:1A2B3C4D", "Annual Report");
    let before = entries(&bytes);
    let after = entries(&edited);
    let names = |list: &[(String, Vec<u8>, u32, u64)]| {
        list.iter()
            .map(|(name, ..)| name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&before), names(&after));
    assert_eq!(before[0].0, "[Content_Types].xml");
    let changed: Vec<&str> = before
        .iter()
        .zip(&after)
        .filter(|(old, new)| old != new)
        .map(|(old, _)| old.0.as_str())
        .collect();
    assert_eq!(changed, ["word/document.xml"]);
    let image = after
        .iter()
        .find(|(name, ..)| name == "word/media/image1.png")
        .expect("image entry");
    assert_eq!(image.1, IMAGE_BYTES);
    assert_eq!(image.3, IMAGE_BYTES.len() as u64);
}

#[test]
fn unknown_markup_and_prologue_survive_an_edit() {
    let edited = replace(&fixture(), "pid:2B3C4D5E", "Revenue fell");
    let xml = part(&edited, "word/document.xml");
    let prologue_end = DOCUMENT_XML.find("<w:body>").expect("body");
    assert!(xml.starts_with(&DOCUMENT_XML[..prologue_end]));
    for marker in [
        r#"<w:bookmarkStart w:id="0" w:name="intro"/>"#,
        r#"<w:bookmarkEnd w:id="0"/>"#,
        "<mc:AlternateContent>",
        r#"<a:blip r:embed="rId9"/>"#,
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#,
    ] {
        assert!(xml.contains(marker), "missing {marker}");
    }
    let projection = project(&edited);
    assert_eq!(text_of(&projection, "pid:2B3C4D5E"), "Revenue fell");
    let image = projection
        .find(&super::docx_fixture::node_id("pid:4D5E6F70/img[0]"))
        .expect("image node");
    assert_eq!(image.kind, NodeKind::Image);
    assert_eq!(image.attrs.get("rel_id").map(String::as_str), Some("rId9"));
}

#[test]
fn run_formatting_survives_replace_text() {
    let edited = replace(&fixture(), "pid:2B3C4D5E", "Margins widened");
    let xml = part(&edited, "word/document.xml");
    assert!(xml.contains("<w:r><w:rPr><w:b/></w:rPr><w:t>Margins widened</w:t></w:r>"));
    let html = DocxCodec.preview_html(&edited).expect("preview");
    assert!(html.contains("<strong>Margins widened</strong>"), "{html}");
}

#[test]
fn replace_text_encodes_breaks_tabs_and_edge_spaces() {
    let edited = replace(&fixture(), "pid:1A2B3C4D", " a\tb\nc ");
    let xml = part(&edited, "word/document.xml");
    assert!(xml.contains(
        r#"<w:t xml:space="preserve"> a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t xml:space="preserve">c </w:t>"#
    ));
    assert_eq!(text_of(&project(&edited), "pid:1A2B3C4D"), " a\tb\nc ");
}

#[test]
fn projection_reads_headings_lists_tables_and_entities() {
    let projection = project(&fixture());
    assert_eq!(projection.format, DocumentFormat::Docx);
    let kinds: Vec<NodeKind> = projection.root.iter().map(|node| node.kind).collect();
    assert_eq!(
        kinds,
        [
            NodeKind::Heading { level: 1 },
            NodeKind::Paragraph,
            NodeKind::ListItem,
            NodeKind::Paragraph,
            NodeKind::Table,
            NodeKind::Paragraph,
        ]
    );
    assert_eq!(
        text_of(&projection, "pid:2B3C4D5E"),
        "Revenue grew strongly."
    );
    assert_eq!(text_of(&projection, "body/tbl[0]/tr[0]/tc[1]"), "Total");
    assert_eq!(text_of(&projection, "pid:6F708192"), "Total");
    assert_eq!(
        text_of(&projection, "pid:708192A3"),
        "Prepared for {{client}} by Ops & Finance."
    );
    let plain = DocxCodec.plain_text(&fixture()).expect("plain");
    assert!(plain.contains("First point\nCaption {{client}}\nRegion\tTotal\nPrepared"));
}

#[test]
fn positional_ids_without_para_ids() {
    let xml = DOCUMENT_XML.replace(" w14:paraId=", " w14:other=");
    let projection = project(&package_with(&xml));
    let ids: Vec<&str> = projection.nodes().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"body/p[0]"));
    assert!(ids.contains(&"body/p[4]"));
    assert!(ids.contains(&"body/tbl[0]/tr[0]/tc[0]/p[0]"));
}

#[test]
fn instantiate_fills_placeholders_in_one_run_and_across_runs() {
    let mut params = TemplateParams::default();
    params.values.insert("client".to_owned(), "Acme".to_owned());
    let filled = DocxCodec
        .instantiate(&fixture(), &params)
        .expect("instantiate");
    let projection = project(&filled);
    assert_eq!(text_of(&projection, "pid:4D5E6F70"), "Caption Acme");
    assert_eq!(
        text_of(&projection, "pid:708192A3"),
        "Prepared for Acme by Ops & Finance."
    );
    let xml = part(&filled, "word/document.xml");
    assert!(xml.contains("<mc:AlternateContent>"));
    assert!(!xml.contains("{{"));
}

#[test]
fn blank_docx_projects_one_empty_paragraph() {
    let blank = blank_docx().expect("blank");
    assert_eq!(blank, blank_docx().expect("blank"));
    let projection = project(&blank);
    assert_eq!(projection.root.len(), 1);
    let node = &projection.root[0];
    assert_eq!(node.id.as_str(), "body/p[0]");
    assert_eq!(node.kind, NodeKind::Paragraph);
    assert_eq!(node.text.as_deref(), Some(""));
    assert_eq!(entries(&blank)[0].0, "[Content_Types].xml");
}

#[test]
fn preview_renders_semantic_html() {
    let html = DocxCodec.preview_html(&fixture()).expect("preview");
    for tag in [
        "<h1>Quarterly Report</h1>",
        "<ul><li>First point</li></ul>",
        "<table>",
        "<td>",
    ] {
        assert!(html.contains(tag), "missing {tag} in {html}");
    }
    assert!(html.contains("Ops &amp; Finance"));
    assert!(html.contains("<em>client</em>"));
}

#[test]
fn codec_is_registered_for_docx() {
    let codec = systemprompt_provider_contracts::documents::codec_for(DocumentFormat::Docx)
        .expect("registered docx codec");
    assert_eq!(codec.formats(), [DocumentFormat::Docx]);
}
