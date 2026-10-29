use systemprompt_files::documents::codecs::MarkdownCodec;
use systemprompt_files::documents::codecs::docx::{DocxCodec, blank_docx};
use systemprompt_files::documents::codecs::pptx::PptxCodec;
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, NodeKind, NodeTarget};

use super::docx_fixture::{DOCUMENT_XML, fixture, package_with, part, replace};
use super::pptx_fixture::{agenda_slide, rebuild, two_slide_deck};
use super::{SAMPLE_MD, id};

fn style_hash(codec: &dyn DocumentCodec, bytes: &[u8], node: &str) -> String {
    codec
        .style_hashes(bytes)
        .expect("style hashes")
        .into_iter()
        .find(|(leaf, _)| leaf.as_str() == node)
        .map(|(_, hash)| hash)
        .expect("leaf hash")
}

fn copy_into_blank(node: &str) -> Vec<u8> {
    let source = fixture();
    let base = DocxCodec.project(&source).expect("project");
    let fragment = DocxCodec
        .copy_node(&source, &base, &id(node))
        .expect("copy");
    assert!(fragment.native.is_some());
    let target = blank_docx().expect("blank");
    let target_base = DocxCodec.project(&target).expect("project");
    let anchor = target_base.root[0].id.clone();
    DocxCodec
        .insert_fragment(&target, &target_base, &NodeTarget::Id(anchor), &fragment)
        .expect("insert")
}

#[test]
fn docx_style_hash_tracks_run_formatting_and_text() {
    let bytes = fixture();
    let node = "pid:2B3C4D5E";
    let original = style_hash(&DocxCodec, &bytes, node);
    assert_eq!(original, style_hash(&DocxCodec, &fixture(), node));
    let unbolded = package_with(&DOCUMENT_XML.replacen("<w:rPr><w:b/></w:rPr>", "", 1));
    assert_ne!(original, style_hash(&DocxCodec, &unbolded, node));
    assert_eq!(
        style_hash(&DocxCodec, &bytes, "pid:3C4D5E6F"),
        style_hash(&DocxCodec, &unbolded, "pid:3C4D5E6F")
    );
    let retexted = replace(&bytes, node, "Revenue fell");
    assert_ne!(original, style_hash(&DocxCodec, &retexted, node));
}

#[test]
fn docx_table_copy_inserts_with_its_cells() {
    let edited = copy_into_blank("body/tbl[0]");
    let projection = DocxCodec.project(&edited).expect("project");
    let table = projection
        .root
        .iter()
        .find(|node| node.kind == NodeKind::Table)
        .expect("table");
    let cells: Vec<String> = projection
        .nodes()
        .filter(|node| {
            node.kind == NodeKind::Cell && node.id.as_str().starts_with(table.id.as_str())
        })
        .filter_map(|node| node.text.clone())
        .collect();
    assert_eq!(cells, ["Region", "Total"]);
    assert!(!part(&edited, "word/document.xml").contains("w14:paraId"));
}

#[test]
fn docx_paragraph_copy_keeps_run_properties() {
    let edited = copy_into_blank("pid:2B3C4D5E");
    let xml = part(&edited, "word/document.xml");
    assert!(xml.contains("<w:rPr><w:b/></w:rPr><w:t>Revenue grew</w:t>"));
    let projection = DocxCodec.project(&edited).expect("project");
    assert!(
        projection
            .nodes()
            .any(|node| node.text.as_deref() == Some("Revenue grew strongly."))
    );
}

#[test]
fn docx_copy_refuses_relationship_markup() {
    let bytes = fixture();
    let base = DocxCodec.project(&bytes).expect("project");
    let refused = DocxCodec.copy_node(&bytes, &base, &id("pid:4D5E6F70"));
    assert!(matches!(refused, Err(CodecError::UnsupportedOp { .. })));
    let nested = DocxCodec.copy_node(&bytes, &base, &id("pid:5E6F7081"));
    assert!(matches!(nested, Err(CodecError::UnsupportedOp { .. })));
}

#[test]
fn pptx_style_hash_tracks_bold_only_changes() {
    let bytes = two_slide_deck();
    let node = "slide[2]/shape[1]/p[0]";
    let bolded = rebuild(
        &bytes,
        &[(
            "ppt/slides/slide1.xml",
            agenda_slide().replacen("i=\"1\"", "i=\"1\" b=\"1\"", 1),
        )],
    );
    let before = PptxCodec.project(&bytes).expect("project");
    let after = PptxCodec.project(&bolded).expect("project");
    assert_eq!(
        before.find(&id(node)).map(|n| n.hash8.clone()),
        after.find(&id(node)).map(|n| n.hash8.clone())
    );
    assert_eq!(
        style_hash(&PptxCodec, &bytes, node),
        style_hash(&PptxCodec, &two_slide_deck(), node)
    );
    assert_ne!(
        style_hash(&PptxCodec, &bytes, node),
        style_hash(&PptxCodec, &bolded, node)
    );
}

#[test]
fn pptx_paragraph_copy_inserts_after_an_anchor_paragraph() {
    let bytes = two_slide_deck();
    let base = PptxCodec.project(&bytes).expect("project");
    let fragment = PptxCodec
        .copy_node(&bytes, &base, &id("slide[2]/shape[1]/p[0]"))
        .expect("copy");
    let edited = PptxCodec
        .insert_fragment(
            &bytes,
            &base,
            &NodeTarget::Id(id("slide[1]/shape[0]/p[0]")),
            &fragment,
        )
        .expect("insert");
    let projection = PptxCodec.project(&edited).expect("project");
    let copied = projection
        .find(&id("slide[1]/shape[0]/p[1]"))
        .expect("copied");
    assert_eq!(copied.text.as_deref(), Some("Revenue & costs"));
    assert!(part(&edited, "ppt/slides/slide2.xml").contains("i=\"1\""));
}

#[test]
fn markdown_default_copy_refuses_containers_and_copies_text() {
    let base = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let container = base
        .nodes()
        .find(|node| !node.children.is_empty())
        .expect("container");
    let refused = MarkdownCodec.copy_node(SAMPLE_MD, &base, &container.id);
    assert!(matches!(refused, Err(CodecError::UnsupportedOp { .. })));
    let fragment = MarkdownCodec
        .copy_node(SAMPLE_MD, &base, &id("line[1]"))
        .expect("copy");
    assert!(fragment.native.is_none());
    let text = fragment.text.clone().expect("text");
    let last = base
        .nodes()
        .filter(|node| node.id.as_str().starts_with("line["))
        .last()
        .expect("last line")
        .id
        .clone();
    let edited = MarkdownCodec
        .insert_fragment(SAMPLE_MD, &base, &NodeTarget::Id(last), &fragment)
        .expect("insert");
    let out = String::from_utf8(edited).expect("utf8");
    assert_eq!(out.matches(text.as_str()).count(), 2);
}
