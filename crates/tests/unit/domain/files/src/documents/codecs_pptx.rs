use systemprompt_files::documents::codecs::pptx::{PptxCodec, blank_pptx};
use systemprompt_provider_contracts::documents::{
    DocumentCodec, DocumentFormat, NodeKind, TemplateParams, codec_for, hash8,
};

use super::pptx_fixture::{hash_of, node, part, project, two_slide_deck};

#[test]
fn blank_deck_projects_one_slide_with_an_empty_title() {
    let projection = project(&blank_pptx().expect("blank"));
    assert_eq!(projection.format, DocumentFormat::Pptx);
    assert_eq!(projection.root.len(), 1);
    let slide = &projection.root[0];
    assert_eq!(slide.id.as_str(), "slide[1]");
    assert_eq!(slide.kind, NodeKind::Slide);
    assert_eq!(slide.text.as_deref(), Some(""));
    assert_eq!(
        slide.attrs.get("layout").map(String::as_str),
        Some("Title and Content")
    );
    let title = projection
        .find(&node("slide[1]/shape[0]"))
        .expect("title shape");
    assert_eq!(
        title.attrs.get("placeholder").map(String::as_str),
        Some("title")
    );
}

#[test]
fn slide_order_follows_the_slide_id_list_not_part_names() {
    let projection = project(&two_slide_deck());
    let parts: Vec<&str> = projection
        .root
        .iter()
        .map(|slide| {
            slide
                .attrs
                .get("part")
                .map(String::as_str)
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(parts, ["ppt/slides/slide2.xml", "ppt/slides/slide1.xml"]);
    assert_eq!(projection.root[0].text.as_deref(), Some("Intro"));
    assert_eq!(projection.root[1].text.as_deref(), Some("Agenda"));
}

#[test]
fn projection_ids_texts_and_hashes() {
    let projection = project(&two_slide_deck());
    let first = projection
        .find(&node("slide[2]/shape[1]/p[0]"))
        .expect("paragraph");
    assert_eq!(first.kind, NodeKind::Paragraph);
    assert_eq!(first.text.as_deref(), Some("Revenue & costs"));
    assert_eq!(first.hash8, hash8("Revenue & costs"));
    assert_eq!(first.attrs.get("bullet").map(String::as_str), Some("true"));
    let second = projection
        .find(&node("slide[2]/shape[1]/p[1]"))
        .expect("paragraph");
    assert_eq!(second.text.as_deref(), Some("Hiring\nplan"));
    let shape = projection.find(&node("slide[2]/shape[1]")).expect("shape");
    assert_eq!(
        shape.attrs.get("name").map(String::as_str),
        Some("Content 2")
    );
    assert_eq!(
        shape.attrs.get("placeholder").map(String::as_str),
        Some("obj")
    );
    assert_eq!(
        shape.hash8,
        hash8(&format!("{} {}", first.hash8, second.hash8))
    );
    let cell = projection
        .find(&node("slide[2]/shape[3]/tbl/r[0]/c[1]"))
        .expect("cell");
    assert_eq!(cell.kind, NodeKind::Cell);
    assert_eq!(cell.text.as_deref(), Some("10"));
    let image = projection.find(&node("slide[2]/shape[4]")).expect("image");
    assert_eq!(image.kind, NodeKind::Image);
    assert_eq!(image.attrs.get("rel_id").map(String::as_str), Some("rId2"));
    let notes = projection
        .find(&node("slide[2]/notes/p[0]"))
        .expect("notes");
    assert_eq!(notes.text.as_deref(), Some("Speak slowly"));
    assert!(projection.find(&node("slide[1]/notes/p[0]")).is_none());
}

#[test]
fn slide_hash_changes_with_its_text() {
    let deck = two_slide_deck();
    let before = hash_of(&project(&deck), "slide[2]");
    let edited = super::pptx_fixture::rebuild(
        &deck,
        &[(
            "ppt/slides/slide1.xml",
            part(&deck, "ppt/slides/slide1.xml").replace("Hiring", "Firing"),
        )],
    );
    assert_ne!(before, hash_of(&project(&edited), "slide[2]"));
    assert_eq!(
        hash_of(&project(&deck), "slide[1]"),
        hash_of(&project(&edited), "slide[1]")
    );
}

#[test]
fn plain_text_lists_slides_paragraphs_and_notes() {
    let text = PptxCodec.plain_text(&two_slide_deck()).expect("text");
    assert!(text.starts_with("# Slide 1\nIntro\n"));
    assert!(text.contains("# Slide 2\nAgenda\nRevenue & costs\nHiring\nplan\n"));
    assert!(text.contains("Q1 | 10\n"));
    assert!(text.contains("## Notes\nSpeak slowly\n"));
}

#[test]
fn preview_renders_sections_lists_and_tables() {
    let html = PptxCodec.preview_html(&two_slide_deck()).expect("preview");
    assert!(html.contains("<section data-slide=\"1\"><h2>Intro</h2></section>"));
    assert!(
        html.contains("<section data-slide=\"2\"><h2>Agenda</h2><ul><li>Revenue &amp; costs</li>")
    );
    assert!(html.contains("<table><tr><td>Q1</td><td>10</td></tr></table>"));
    assert!(!html.contains("style="));
}

#[test]
fn instantiate_fills_split_and_whole_placeholders() {
    let mut params = TemplateParams::default();
    params
        .values
        .insert("customer".to_owned(), "Acme <EU>".to_owned());
    params.values.insert("name".to_owned(), "Ada".to_owned());
    let filled = PptxCodec
        .instantiate(&two_slide_deck(), &params)
        .expect("instantiate");
    let paragraph = project(&filled)
        .find(&node("slide[2]/shape[2]/p[0]"))
        .and_then(|para| para.text.clone());
    assert_eq!(paragraph.as_deref(), Some("Dear Acme <EU>, hello Ada"));
    assert!(part(&filled, "ppt/slides/slide1.xml").contains("Acme &lt;EU&gt;"));
}

#[test]
fn instantiate_fills_a_placeholder_inside_one_run_in_place() {
    let deck = two_slide_deck();
    let templated = super::pptx_fixture::rebuild(
        &deck,
        &[(
            "ppt/slides/slide2.xml",
            part(&deck, "ppt/slides/slide2.xml").replace(">Intro<", ">Hi {{name}}<"),
        )],
    );
    let mut params = TemplateParams::default();
    params.values.insert("name".to_owned(), "Ada".to_owned());
    let filled = PptxCodec
        .instantiate(&templated, &params)
        .expect("instantiate");
    let xml = part(&filled, "ppt/slides/slide2.xml");
    assert!(xml.contains("<a:r><a:rPr lang=\"en-US\"/><a:t>Hi Ada</a:t></a:r>"));
}

#[test]
fn codec_is_registered_for_pptx() {
    let codec = codec_for(DocumentFormat::Pptx).expect("registered");
    assert_eq!(codec.formats(), &[DocumentFormat::Pptx]);
}
