use systemprompt_files::documents::codecs::docx::{blank_docx, import_rich, materialise_rich};
use systemprompt_files::documents::model::layout::{Align, CalloutTone, Indent, ParaAttrs};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};

use super::docx_fixture::part;

fn written(blocks: Vec<Block>) -> (Vec<u8>, String) {
    let out = materialise_rich(&blank_docx().expect("blank"), &RichText::of(blocks))
        .expect("materialise");
    let document = part(&out, "word/document.xml");
    (out, document)
}

fn with_attrs(id: &str, attrs: ParaAttrs) -> Block {
    let mut block = Block::new(id, BlockKind::Paragraph, id);
    block.attrs = attrs;
    block
}

fn indent(left: u32, right: u32, first_line: i32) -> Option<Indent> {
    Some(Indent {
        left,
        right,
        first_line,
    })
}

#[test]
fn paragraph_attributes_write_style_spacing_indent_and_alignment() {
    let blocks = vec![
        with_attrs(
            "hang",
            ParaAttrs {
                style: Some("Body \"Text\"".to_owned()),
                keep_with_next: true,
                space_before: Some(6),
                space_after: Some(12),
                indent: indent(36, 18, -18),
                align: Some(Align::Justify),
            },
        ),
        with_attrs(
            "first",
            ParaAttrs {
                space_after: Some(3),
                indent: indent(0, 0, 9),
                align: Some(Align::Center),
                ..ParaAttrs::default()
            },
        ),
        with_attrs(
            "flat",
            ParaAttrs {
                indent: indent(10, 0, 0),
                align: Some(Align::Right),
                ..ParaAttrs::default()
            },
        ),
        with_attrs(
            "left",
            ParaAttrs {
                align: Some(Align::Left),
                ..ParaAttrs::default()
            },
        ),
    ];
    let (out, document) = written(blocks);
    assert!(document.contains(
        "<w:pPr><w:pStyle w:val=\"Body &quot;Text&quot;\"/><w:keepNext/><w:spacing w:before=\"120\" w:after=\"240\"/><w:ind w:left=\"720\" w:right=\"360\" w:hanging=\"360\"/><w:jc w:val=\"both\"/></w:pPr>"
    ));
    assert!(document.contains(
        "<w:pPr><w:spacing w:after=\"60\"/><w:ind w:left=\"0\" w:right=\"0\" w:firstLine=\"180\"/><w:jc w:val=\"center\"/></w:pPr>"
    ));
    assert!(document.contains("<w:ind w:left=\"200\" w:right=\"0\"/><w:jc w:val=\"right\"/>"));
    assert!(document.contains("<w:jc w:val=\"left\"/>"));

    let back = import_rich(&out).expect("import");
    let attrs = |id: &str| {
        back.blocks
            .iter()
            .find(|block| block.text == id)
            .map(|block| block.attrs.clone())
            .expect("block")
    };
    assert_eq!(attrs("hang").indent, indent(36, 18, -18));
    assert_eq!(attrs("hang").align, Some(Align::Justify));
    assert!(attrs("hang").keep_with_next);
    assert_eq!(attrs("first").space_after, Some(3));
    assert_eq!(attrs("flat").align, Some(Align::Right));
}

#[test]
fn callouts_take_their_tone_style_and_border() {
    let tones = [
        (CalloutTone::Info, "2F6FDE"),
        (CalloutTone::Success, "2E8B57"),
        (CalloutTone::Warning, "C98A00"),
        (CalloutTone::Danger, "C0392B"),
        (CalloutTone::Note, "6B7280"),
    ];
    let blocks = tones
        .iter()
        .map(|(tone, _)| Block::new("c", BlockKind::Callout { tone: *tone }, "heads up"))
        .collect();
    let (out, document) = written(blocks);
    for (_, rule) in tones {
        assert!(document.contains(&format!("w:color=\"{rule}\"")), "{rule}");
    }
    let back = import_rich(&out).expect("import");
    let read: Vec<BlockKind> = back
        .blocks
        .iter()
        .filter(|block| block.text == "heads up")
        .map(|block| block.kind.clone())
        .collect();
    assert_eq!(
        read,
        tones
            .iter()
            .map(|(tone, _)| BlockKind::Callout { tone: *tone })
            .collect::<Vec<_>>()
    );
}

#[test]
fn tasks_code_breaks_dividers_and_a_contents_field_are_written() {
    let task = |checked: bool, text: &str| {
        Block::new(
            text,
            BlockKind::ListItem {
                level: 0,
                ordered: false,
                numbering: None,
                checked: Some(checked),
            },
            text,
        )
    };
    let blocks = vec![
        task(true, "done"),
        task(false, "todo"),
        Block::new(
            "code",
            BlockKind::Code {
                language: Some("rust".to_owned()),
            },
            "fn main() {}",
        ),
        Block::new("pb", BlockKind::PageBreak, ""),
        Block::new("hr", BlockKind::Divider, ""),
        Block::new("toc", BlockKind::Toc { levels: 2 }, ""),
        Block::new("q", BlockKind::Quote, "quoted"),
    ];
    let (out, document) = written(blocks);
    assert!(document.contains("\u{2612} "));
    assert!(document.contains("\u{2610} "));
    assert!(document.contains("<w:pStyle w:val=\"Code\"/>"));
    assert!(document.contains("fn main() {}"));
    assert!(document.contains("w:type=\"page\""));
    assert!(document.contains(" TOC \\o \"1-2\" \\h \\z \\u "));
    assert!(document.contains("<w:pStyle w:val=\"Quote\"/>"));
    assert!(part(&out, "word/settings.xml").contains("updateFields"));

    let back = import_rich(&out).expect("import");
    let kinds: Vec<&BlockKind> = back.blocks.iter().map(|block| &block.kind).collect();
    assert!(kinds.contains(&&BlockKind::PageBreak));
    assert!(kinds.contains(&&BlockKind::Divider));
    assert!(kinds.contains(&&BlockKind::Toc { levels: 2 }));
    assert!(kinds.contains(&&BlockKind::Quote));
    let items = back
        .blocks
        .iter()
        .filter(|block| matches!(block.kind, BlockKind::ListItem { .. }))
        .count();
    assert_eq!(items, 2);
}
