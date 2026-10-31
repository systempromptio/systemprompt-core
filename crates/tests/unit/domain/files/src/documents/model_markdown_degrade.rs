use systemprompt_files::documents::model::document::{DocumentSettings, Theme};
use systemprompt_files::documents::model::layout::{
    Align, CalloutTone, CellProps, Column, ImageWrap, ParaAttrs, Section, TableProps,
};
use systemprompt_files::documents::model::markdown::{degrade, import, materialise};
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind, RichText};
use systemprompt_provider_contracts::documents::CodecError;

fn para(id: &str, text: &str) -> Block {
    Block::new(id, BlockKind::Paragraph, text)
}

fn cell(text: &str) -> RichText {
    RichText::of(vec![para("c", text)])
}

fn refusal(model: &RichText) -> String {
    match materialise(model).expect_err("refused") {
        CodecError::Refused { detail } => detail,
        other => panic!("unexpected error {other:?}"),
    }
}

fn styled_table() -> Block {
    Block::new(
        "t1",
        BlockKind::Table {
            rows: vec![vec![cell("a"), cell("b")]],
            props: TableProps {
                columns: vec![Column { width: Some(40) }],
                header_rows: 2,
                cells: vec![vec![CellProps {
                    shade: Some("FF0000".to_owned()),
                    ..CellProps::default()
                }]],
                style: Some("Grid".to_owned()),
            },
        },
        "",
    )
}

fn sized_image() -> Block {
    Block::new(
        "i1",
        BlockKind::Image {
            blob: "https://cdn.example.com/a.png".to_owned(),
            alt: "A".to_owned(),
            width: Some(10),
            height: None,
            caption: None,
            wrap: ImageWrap::Square,
        },
        "",
    )
}

fn numbered_item() -> Block {
    Block::new(
        "l1",
        BlockKind::ListItem {
            level: 0,
            ordered: true,
            numbering: Some("num-1".to_owned()),
            checked: None,
        },
        "first",
    )
}

fn aligned() -> Block {
    let mut block = para("p1", "centred");
    block.attrs = ParaAttrs {
        align: Some(Align::Center),
        ..ParaAttrs::default()
    };
    block
}

#[test]
fn degrade_strips_every_construct_markdown_cannot_hold() {
    let model = RichText::of(vec![
        aligned(),
        numbered_item(),
        styled_table(),
        sized_image(),
        Block::new("k1", BlockKind::Toc { levels: 3 }, ""),
    ]);
    let (degraded, warnings) = degrade(&model);
    assert_eq!(warnings.len(), 5);
    for id in ["p1", "l1", "t1", "i1", "k1"] {
        assert!(
            warnings.iter().any(|w| w.contains(id)),
            "{id}: {warnings:?}"
        );
    }
    assert!(degraded.blocks[0].attrs.is_default());
    assert!(matches!(
        degraded.blocks[1].kind,
        BlockKind::ListItem {
            numbering: None,
            ..
        }
    ));
    let plain = TableProps {
        header_rows: 1,
        ..TableProps::default()
    };
    assert!(matches!(
        &degraded.blocks[2].kind,
        BlockKind::Table { props, .. } if *props == plain
    ));
    assert!(matches!(
        degraded.blocks[3].kind,
        BlockKind::Image {
            width: None,
            height: None,
            wrap: ImageWrap::Inline,
            ..
        }
    ));
    assert_eq!(degraded.blocks[4].kind, BlockKind::Divider);
    assert!(materialise(&degraded).is_ok());
}

#[test]
fn degrade_keeps_front_matter_and_warns_about_dropped_settings() {
    let mut model = RichText::of(vec![para("p", "body")]);
    model.document = Some(Box::new(DocumentSettings {
        theme: Some(Theme::default()),
        front_matter: Some("title: X\n".to_owned()),
        ..DocumentSettings::default()
    }));
    let (degraded, warnings) = degrade(&model);
    assert_eq!(
        warnings,
        vec!["left out styles, theme, numbering or page setup".to_owned()]
    );
    let settings = degraded.settings();
    assert_eq!(settings.front_matter.as_deref(), Some("title: X\n"));
    assert!(settings.theme.is_none());
}

#[test]
fn degrade_of_a_plain_model_is_silent_and_drops_empty_settings() {
    let mut model = RichText::of(vec![para("p", "body")]);
    model.document = Some(Box::new(DocumentSettings::default()));
    let (degraded, warnings) = degrade(&model);
    assert!(warnings.is_empty());
    assert!(degraded.document.is_none());
}

#[test]
fn materialise_refuses_each_unholdable_block_by_kind() {
    let cases = [
        (aligned(), "paragraph formatting"),
        (numbered_item(), "numbering definition"),
        (styled_table(), "table widths"),
        (sized_image(), "image size"),
        (Block::new("x", BlockKind::PageBreak, ""), "page break"),
        (
            Block::new(
                "x",
                BlockKind::SectionBreak {
                    section: Box::new(Section::default()),
                },
                "",
            ),
            "section break",
        ),
        (
            Block::new("x", BlockKind::Toc { levels: 2 }, ""),
            "table of contents",
        ),
    ];
    for (block, what) in cases {
        let detail = refusal(&RichText::of(vec![block]));
        assert!(detail.contains(what), "{what}: {detail}");
    }
}

#[test]
fn materialise_refuses_settings_markdown_cannot_hold() {
    let mut model = RichText::of(vec![para("p", "body")]);
    model.document = Some(Box::new(DocumentSettings {
        section: Some(Section::default()),
        ..DocumentSettings::default()
    }));
    assert!(refusal(&model).contains("page setup"));
}

#[test]
fn materialise_writes_every_holdable_kind() {
    let mut captioned = Block::new(
        "i",
        BlockKind::image("pic.png".to_owned(), "Alt".to_owned()),
        "",
    );
    if let BlockKind::Image { caption, .. } = &mut captioned.kind {
        *caption = Some("say \"hi\"".to_owned());
    }
    let model = RichText::of(vec![
        Block::new("h", BlockKind::Heading { level: 3 }, "Title"),
        Block::new(
            "l",
            BlockKind::ListItem {
                level: 1,
                ordered: false,
                numbering: None,
                checked: Some(false),
            },
            "todo",
        ),
        Block::new("q", BlockKind::Quote, "a\nb"),
        Block::new(
            "n",
            BlockKind::Callout {
                tone: CalloutTone::Danger,
            },
            "careful",
        ),
        Block::new("c", BlockKind::Code { language: None }, "x = 1"),
        captioned,
        Block::new(
            "t",
            BlockKind::table(vec![vec![cell("a|b"), cell("c")], vec![cell("d")]]),
            "",
        ),
        Block::new(
            "r",
            BlockKind::Raw {
                source: "<div>raw</div>".to_owned(),
            },
            "",
        ),
        Block::new("d", BlockKind::Divider, ""),
    ]);
    let written = materialise(&model).expect("materialise");
    for expected in [
        "### Title",
        "  - [ ] todo",
        "> a\n> b",
        "> [!CAUTION]\n> careful",
        "```\nx = 1\n```",
        "![Alt](pic.png \"say \\\"hi\\\"\")",
        "| a\\|b | c |\n| --- | --- |\n| d |  |",
        "<div>raw</div>",
        "---",
    ] {
        assert!(written.contains(expected), "{expected:?} in {written}");
    }
}

#[test]
fn inline_marks_render_as_markdown_and_import_back() {
    let mut block = para("p", "bold code link *x*");
    for (start, end, kind) in [
        (0, 4, MarkKind::Bold),
        (5, 9, MarkKind::Code),
        (
            10,
            14,
            MarkKind::Link {
                href: "https://e.com".to_owned(),
            },
        ),
        (15, 18, MarkKind::Strike),
    ] {
        block.marks.push(Mark { start, end, kind });
    }
    let written = materialise(&RichText::of(vec![block])).expect("materialise");
    assert!(written.contains("**bold**"), "{written}");
    assert!(written.contains("`code`"), "{written}");
    assert!(written.contains("[link](https://e.com)"), "{written}");
    assert!(written.contains("~~\\*x\\*~~"), "{written}");
    assert_eq!(import(&written).blocks[0].text, "bold code link *x*");
}
