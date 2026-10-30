use systemprompt_files::documents::codecs::docx::{blank_docx, import_rich, materialise_rich};
use systemprompt_files::documents::model::layout::{
    ImageWrap, Margins, Orientation, PageSize, Section,
};
use systemprompt_files::documents::model::{
    Block, BlockKind, DocumentSettings, Mark, MarkKind, RichText,
};

use super::docx_fixture::{fixture, part};

fn running(text: &str) -> RichText {
    RichText::of(vec![Block::new(
        format!("r-{text}"),
        BlockKind::Paragraph,
        text,
    )])
}

fn full_section() -> Section {
    Section {
        size: Some(PageSize {
            width: 842,
            height: 595,
        }),
        orientation: Orientation::Landscape,
        title_page: false,
        margins: Some(Margins {
            top: 36,
            right: 54,
            bottom: 36,
            left: 54,
        }),
        header: Some(running("Default head")),
        footer: Some(running("Default foot")),
        first_page_header: Some(running("First head")),
        first_page_footer: Some(running("First foot")),
        even_header: Some(running("Even head")),
        even_footer: Some(running("Even foot")),
    }
}

fn with_section(blocks: Vec<Block>, section: Section) -> RichText {
    let mut model = RichText::of(blocks);
    model.document = Some(Box::new(DocumentSettings {
        section: Some(section),
        ..DocumentSettings::default()
    }));
    model
}

fn body() -> Vec<Block> {
    vec![Block::new("p1", BlockKind::Paragraph, "Body text")]
}

fn final_section(bytes: &[u8]) -> Section {
    import_rich(bytes)
        .expect("import")
        .settings()
        .section
        .expect("section")
}

#[test]
fn six_running_parts_round_trip_with_page_setup() {
    let out = materialise_rich(
        &blank_docx().expect("blank"),
        &with_section(body(), full_section()),
    )
    .expect("materialise");
    let section = final_section(&out);
    let text = |part: Option<RichText>| part.expect("part").blocks[0].text.clone();
    assert_eq!(text(section.header.clone()), "Default head");
    assert_eq!(text(section.footer.clone()), "Default foot");
    assert_eq!(text(section.first_page_header.clone()), "First head");
    assert_eq!(text(section.first_page_footer.clone()), "First foot");
    assert_eq!(text(section.even_header.clone()), "Even head");
    assert_eq!(text(section.even_footer.clone()), "Even foot");
    assert_eq!(section.orientation, Orientation::Landscape);
    assert_eq!(
        section.size,
        Some(PageSize {
            width: 842,
            height: 595
        })
    );
    assert_eq!(
        section.margins,
        Some(Margins {
            top: 36,
            right: 54,
            bottom: 36,
            left: 54
        })
    );
    assert!(section.title_page);
    let document = part(&out, "word/document.xml");
    assert!(document.contains("<w:pgSz w:w=\"16840\" w:h=\"11900\" w:orient=\"landscape\"/>"));
    assert!(document.contains("<w:titlePg/>"));
    for kind in ["default", "first", "even"] {
        assert!(document.contains(&format!("<w:headerReference w:type=\"{kind}\"")));
        assert!(document.contains(&format!("<w:footerReference w:type=\"{kind}\"")));
    }
}

#[test]
fn running_parts_get_content_types_relationships_and_their_own_roots() {
    let out = materialise_rich(
        &blank_docx().expect("blank"),
        &with_section(body(), full_section()),
    )
    .expect("materialise");
    let types = part(&out, "[Content_Types].xml");
    let rels = part(&out, "word/_rels/document.xml.rels");
    let names: Vec<String> = super::docx_fixture::entries(&out)
        .into_iter()
        .map(|(name, ..)| name)
        .filter(|name| name.starts_with("word/sp-"))
        .collect();
    assert_eq!(names.len(), 6);
    for name in &names {
        let short = name.trim_start_matches("word/");
        assert!(rels.contains(&format!("Target=\"{short}\"")), "{short}");
        assert!(types.contains(&format!("PartName=\"/{name}\"")), "{name}");
        let xml = part(&out, name);
        let root = if short.starts_with("sp-header") {
            "<w:hdr "
        } else {
            "<w:ftr "
        };
        assert!(xml.contains(root), "{name}");
    }
    assert!(types.contains("wordprocessingml.header+xml"));
    assert!(types.contains("wordprocessingml.footer+xml"));
}

#[test]
fn even_headers_and_a_toc_create_a_settings_part_when_the_base_has_none() {
    let mut blocks = body();
    blocks.push(Block::new("toc", BlockKind::Toc { levels: 3 }, ""));
    let section = Section {
        even_footer: Some(running("Even")),
        ..Section::default()
    };
    let out = materialise_rich(
        &blank_docx().expect("blank"),
        &with_section(blocks, section),
    )
    .expect("materialise");
    let settings = part(&out, "word/settings.xml");
    assert!(settings.contains("<w:evenAndOddHeaders/><w:updateFields w:val=\"true\"/>"));
    assert!(part(&out, "word/_rels/document.xml.rels").contains("Target=\"settings.xml\""));
    assert!(part(&out, "[Content_Types].xml").contains("wordprocessingml.settings+xml"));
    assert!(!part(&out, "word/document.xml").contains("<w:titlePg/>"));
}

#[test]
fn settings_are_inserted_into_an_existing_part_in_schema_order() {
    let base = materialise_rich(&fixture(), &import_rich(&fixture()).expect("import"))
        .expect("materialise");
    let mut model = import_rich(&base).expect("import");
    model
        .blocks
        .push(Block::new("toc", BlockKind::Toc { levels: 2 }, ""));
    model.document.get_or_insert_with(Box::default).section = Some(Section {
        even_header: Some(running("Even")),
        ..Section::default()
    });
    let out = materialise_rich(&base, &model).expect("materialise");
    let settings = part(&out, "word/settings.xml");
    assert!(settings.starts_with("<w:settings xmlns:w=\"urn:x\"><w:zoom w:percent=\"100\"/>"));
    assert!(settings.contains("<w:evenAndOddHeaders/>"));
    assert!(settings.contains("<w:updateFields w:val=\"true\"/>"));
    let again = materialise_rich(&out, &import_rich(&out).expect("import")).expect("again");
    assert_eq!(
        part(&again, "word/settings.xml")
            .matches("<w:updateFields")
            .count(),
        1
    );
}

#[test]
fn a_section_break_writes_its_own_sect_pr_and_reads_back() {
    let first = Section {
        title_page: true,
        header: Some(running("Part one")),
        ..Section::default()
    };
    let blocks = vec![
        Block::new("p1", BlockKind::Paragraph, "Chapter one"),
        Block::new(
            "s1",
            BlockKind::SectionBreak {
                section: Box::new(first),
            },
            "",
        ),
        Block::new("p2", BlockKind::Paragraph, "Chapter two"),
    ];
    let out = materialise_rich(&blank_docx().expect("blank"), &RichText::of(blocks))
        .expect("materialise");
    let model = import_rich(&out).expect("import");
    let section = model
        .blocks
        .iter()
        .find_map(|block| match &block.kind {
            BlockKind::SectionBreak { section } => Some(section.clone()),
            _ => None,
        })
        .expect("a section break");
    assert!(section.title_page);
    assert_eq!(section.header.expect("header").blocks[0].text, "Part one");
}

#[test]
fn a_header_keeps_link_text_and_refuses_images() {
    let mut linked = Block::new("h", BlockKind::Paragraph, "See site");
    linked.marks.push(Mark {
        start: 4,
        end: 8,
        kind: MarkKind::Link {
            href: "https://example.com".to_owned(),
        },
    });
    let section = Section {
        header: Some(RichText::of(vec![linked])),
        ..Section::default()
    };
    let out = materialise_rich(
        &blank_docx().expect("blank"),
        &with_section(body(), section),
    )
    .expect("materialise");
    let header = final_section(&out).header.expect("header");
    assert_eq!(header.blocks[0].text, "See site");
    let image = Block::new(
        "img",
        BlockKind::Image {
            blob: "docx-media:media/image1.png".to_owned(),
            alt: String::new(),
            width: None,
            height: None,
            caption: None,
            wrap: ImageWrap::Inline,
        },
        "",
    );
    let refused = Section {
        footer: Some(RichText::of(vec![image])),
        ..Section::default()
    };
    assert!(
        materialise_rich(&fixture(), &with_section(body(), refused)).is_err(),
        "an image in a footer part is refused"
    );
}

#[test]
fn an_empty_header_still_writes_a_paragraph() {
    let section = Section {
        header: Some(RichText::default()),
        ..Section::default()
    };
    let out = materialise_rich(
        &blank_docx().expect("blank"),
        &with_section(body(), section),
    )
    .expect("materialise");
    assert!(part(&out, "word/sp-header1.xml").contains("<w:p/></w:hdr>"));
}
