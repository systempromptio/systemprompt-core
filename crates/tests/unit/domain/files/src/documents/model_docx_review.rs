use systemprompt_files::documents::codecs::docx::{
    MAX_PART_BYTES, blank_docx, import_rich, materialise_rich, materialise_rich_with,
    saved_upload_hashes,
};
use systemprompt_files::documents::collab::ImageOwners;
use systemprompt_files::documents::model::layout::{Margins, Section};
use systemprompt_files::documents::model::{
    Block, BlockKind, EditLevel, Limits, Media, MediaBlob, ModelRejection, RichText, validate,
};
use systemprompt_files::documents::service::held_media_refs;
use systemprompt_identifiers::UserId;
use systemprompt_provider_contracts::documents::{DocumentFormat, content_hash};

use super::docx_fixture::{entries, fixture, package_with, part};
use super::model_docx_roundtrip::{BLOB, media, png, written};

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
        validate(
            &RichText::default(),
            &model,
            EditLevel::Edit,
            &Limits::default()
        ),
        Err(ModelRejection::RawChanged("evil".to_owned()))
    );
}

#[test]
fn a_document_already_holding_an_upload_lets_it_be_placed_again() {
    let markdown = format!("![chart]({BLOB})\n");
    assert_eq!(
        held_media_refs(
            DocumentFormat::Markdown,
            markdown.as_bytes(),
            &[BLOB.to_owned()],
            MAX_PART_BYTES
        ),
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
    let headers = names
        .iter()
        .filter(|name| name.contains("sp-header"))
        .count();
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

const BREAK_XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>"#,
    r#"<w:p><w:r><w:t>Intro</w:t></w:r></w:p>"#,
    r#"<w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rId77"/><w:type w:val="nextPage"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/><w:cols w:num="2" w:space="708"/></w:sectPr></w:pPr></w:p>"#,
    r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>"#,
    r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#,
    "</w:body></w:document>",
);

#[test]
fn an_edited_mid_body_break_keeps_its_untouched_children() {
    let base = package_with(BREAK_XML);
    let mut model = import_rich(&base).expect("import");
    let section = model
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.kind {
            BlockKind::SectionBreak { section } => Some(section),
            _ => None,
        })
        .expect("a section break");
    section.margins = Some(Margins {
        top: 36,
        right: 72,
        bottom: 72,
        left: 72,
    });
    let out = materialise_rich(&base, &model).expect("materialise");
    let xml = part(&out, "word/document.xml");
    assert!(xml.contains("<w:headerReference w:type=\"default\" r:id=\"rId77\"/>"));
    assert!(xml.contains("<w:type w:val=\"nextPage\"/>"));
    assert!(xml.contains("<w:cols w:num=\"2\" w:space=\"708\"/>"));
    assert!(xml.contains("w:top=\"720\" w:right=\"1440\""));
    assert!(xml.contains("w:header=\"708\""));
}

fn with_upload(reference: &str, bytes: Vec<u8>) -> Vec<u8> {
    let image = BlockKind::image(reference.to_owned(), String::new());
    let model = RichText::of(vec![Block::new("img", image, "")]);
    let mut media = Media::new();
    media.insert(
        reference.to_owned(),
        MediaBlob {
            bytes,
            mime: "image/png".to_owned(),
        },
    );
    materialise_rich_with(&blank_docx().expect("blank"), &model, &media).expect("materialise")
}

#[test]
fn a_saved_docx_upload_counts_as_held() {
    let bytes = png(64, 32);
    let hash = content_hash(&bytes).as_str().to_owned();
    let saved = with_upload(&hash, bytes);
    let wanted = [hash.clone()];
    assert!(held_media_refs(DocumentFormat::Docx, &saved, &wanted, MAX_PART_BYTES).contains(&hash));
}

#[test]
fn an_oversized_saved_upload_part_is_skipped() {
    let bytes = png(64, 32);
    let hash = content_hash(&bytes).as_str().to_owned();
    let saved = with_upload(&hash, bytes);
    let wanted = [hash.clone()];
    assert_eq!(
        saved_upload_hashes(&saved, &wanted, MAX_PART_BYTES),
        vec![hash.clone()]
    );
    assert!(saved_upload_hashes(&saved, &wanted, 8).is_empty());
    assert!(saved_upload_hashes(&saved, &[], MAX_PART_BYTES).is_empty());
}

#[test]
fn a_part_named_for_another_upload_is_not_held() {
    let other = "1111111111111111111111111111111111111111111111111111111111111111";
    let crafted = with_upload(other, png(8, 8));
    let wanted = [other.to_owned()];
    assert!(saved_upload_hashes(&crafted, &wanted, MAX_PART_BYTES).is_empty());
    assert!(
        !held_media_refs(DocumentFormat::Docx, &crafted, &wanted, MAX_PART_BYTES)
            .contains(&wanted[0])
    );
}

#[test]
fn an_image_keeps_the_editor_the_log_records_as_its_inserter() {
    let alice = UserId::new("alice");
    let bob = UserId::new("bob");
    let typed = RichText::of(vec![Block::new("p1", BlockKind::Paragraph, "Notes")]);
    let mut inserted = typed.clone();
    inserted.blocks.push(Block::new(
        "img",
        BlockKind::image(BLOB.to_owned(), String::new()),
        "",
    ));
    let mut more = inserted.clone();
    more.blocks[0].text = "Notes, edited".to_owned();
    let mut owners = ImageOwners::default();
    owners.record(&typed, &inserted, &alice);
    owners.record(&inserted, &more, &bob);
    assert_eq!(owners.owner(BLOB), Some(&alice));
}

#[test]
fn a_rebuilt_entry_keeps_its_image_owners() {
    let alice = UserId::new("alice");
    let mut inserted = RichText::default();
    inserted.blocks.push(Block::new(
        "img",
        BlockKind::image(BLOB.to_owned(), String::new()),
        "",
    ));
    let mut owners = ImageOwners::default();
    owners.record(&RichText::default(), &inserted, &alice);
    let blob = owners.seal(b"engine-snapshot");
    let (reopened, snapshot) = ImageOwners::open(&blob);
    assert_eq!(snapshot, b"engine-snapshot");
    assert_eq!(reopened, owners);
    let (legacy, raw) = ImageOwners::open(b"engine-snapshot");
    assert_eq!(raw, b"engine-snapshot");
    assert_eq!(legacy.owner(BLOB), None);
}
