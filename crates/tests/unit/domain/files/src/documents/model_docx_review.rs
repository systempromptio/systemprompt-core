use systemprompt_files::documents::codecs::docx::{
    import_rich, materialise_rich, materialise_rich_with,
};
use systemprompt_files::documents::model::layout::{Margins, Section};
use systemprompt_files::documents::model::{
    Block, BlockKind, EditLevel, Limits, ModelRejection, RichText, validate,
};
use systemprompt_files::documents::service::held_media_refs;
use systemprompt_provider_contracts::documents::DocumentFormat;

use super::docx_fixture::{entries, fixture, part};
use super::model_docx_roundtrip::{BLOB, media, written};

fn raw_block(id: &str) -> Block {
    Block::new(
        id,
        BlockKind::Raw {
            source: "<w:p><w:r><w:t>injected</w:t></w:r></w:p>".to_owned(),
        },
        "",
    )
}

#[test]
fn raw_markup_in_a_final_section_header_is_refused() {
    let mut model = import_rich(&written()).expect("import");
    let section = model
        .document
        .as_mut()
        .and_then(|settings| settings.section.as_mut())
        .expect("section");
    section.header = Some(RichText::of(vec![raw_block("evil")]));
    let before = import_rich(&written()).expect("import");
    assert_eq!(
        validate(&before, &model, EditLevel::Edit, &Limits::default()),
        Err(ModelRejection::RawChanged("evil".to_owned()))
    );
}

#[test]
fn raw_markup_in_a_section_break_footer_is_refused() {
    let section = Section {
        footer: Some(RichText::of(vec![raw_block("evil")])),
        ..Section::default()
    };
    let model = RichText::of(vec![Block::new(
        "s1",
        BlockKind::SectionBreak {
            section: Box::new(section),
        },
        "",
    )]);
    assert_eq!(
        validate(&RichText::default(), &model, EditLevel::Edit, &Limits::default()),
        Err(ModelRejection::RawChanged("evil".to_owned()))
    );
}

#[test]
fn a_document_already_holding_an_upload_lets_it_be_placed_again() {
    let markdown = format!("![chart]({BLOB})\n");
    assert_eq!(
        held_media_refs(DocumentFormat::Markdown, markdown.as_bytes()),
        vec![BLOB.to_owned()]
    );
}

fn header_text(model: &mut RichText, text: &str) {
    let header = model
        .document
        .as_mut()
        .and_then(|settings| settings.section.as_mut())
        .and_then(|section| section.header.as_mut())
        .expect("header");
    header.blocks[0].text = text.to_owned();
}

#[test]
fn a_new_header_part_never_overwrites_an_existing_one() {
    let first = written();
    let mut model = import_rich(&first).expect("import");
    header_text(&mut model, "Internal");
    let again = materialise_rich(&first, &model).expect("materialise");
    assert_eq!(
        part(&again, "word/sp-header1.xml"),
        part(&first, "word/sp-header1.xml")
    );
    let names: Vec<String> = entries(&again).into_iter().map(|(name, ..)| name).collect();
    let headers = names.iter().filter(|name| name.contains("sp-header")).count();
    assert_eq!(headers, 2);
    let model = import_rich(&again).expect("reimport");
    let section = model.settings().section.expect("section");
    assert_eq!(section.header.expect("header").blocks[0].text, "Internal");
}

#[test]
fn an_unchanged_footer_keeps_its_reference_when_the_header_changes() {
    let first = written();
    let xml = part(&first, "word/document.xml");
    let footer_ref = xml
        .split("<w:footerReference")
        .nth(1)
        .and_then(|rest| rest.split("/>").next())
        .expect("footer reference")
        .to_owned();
    let mut model = import_rich(&first).expect("import");
    header_text(&mut model, "Internal");
    let again = materialise_rich(&first, &model).expect("materialise");
    assert!(part(&again, "word/document.xml").contains(&footer_ref));
}

#[test]
fn a_margin_change_keeps_the_base_page_size_twips_and_other_children() {
    let base = fixture();
    let mut model = import_rich(&base).expect("import");
    let settings = model.document.as_mut().expect("settings");
    let section = settings.section.as_mut().expect("section");
    section.margins = Some(Margins {
        top: 72,
        right: 72,
        bottom: 72,
        left: 72,
    });
    let out = materialise_rich(&base, &model).expect("materialise");
    let xml = part(&out, "word/document.xml");
    assert!(xml.contains("<w:pgSz w:w=\"11906\" w:h=\"16838\"/>"));
    assert!(xml.contains("<w:pgMar w:top=\"1440\""));
}

#[test]
fn title_page_is_imported_and_kept() {
    let first = written();
    let mut model = import_rich(&first).expect("import");
    let section = model
        .document
        .as_mut()
        .and_then(|settings| settings.section.as_mut())
        .expect("section");
    section.title_page = true;
    let again = materialise_rich(&first, &model).expect("materialise");
    assert!(part(&again, "word/document.xml").contains("<w:titlePg/>"));
    let reread = import_rich(&again).expect("reimport");
    assert!(reread.settings().section.expect("section").title_page);
}

#[test]
fn a_second_drawing_takes_a_fresh_object_id() {
    let first = written();
    let mut model = import_rich(&first).expect("import");
    let mut image = BlockKind::image(BLOB.to_owned(), "Again".to_owned());
    if let BlockKind::Image { width, .. } = &mut image {
        *width = Some(100);
    }
    model.blocks.push(Block::new("new-image", image, ""));
    let again = materialise_rich_with(&first, &model, &media()).expect("materialise");
    let xml = part(&again, "word/document.xml");
    let ids: Vec<&str> = xml
        .split("<wp:docPr id=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(ids.len(), 2);
    assert_eq!(unique.len(), ids.len());
}
