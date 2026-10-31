use systemprompt_files::documents::model::html::{import, materialise};
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind, RichText, segments};

fn keys(block: &Block) -> Vec<&'static str> {
    block.marks.iter().map(|mark| mark.kind.key()).collect()
}

#[test]
fn block_elements_import_to_their_kinds() {
    let model = import(
        "<h2>Head</h2><p>Body</p><blockquote>Q</blockquote><pre>code</pre><hr>\
         <img src=\"a.png\" alt=\"Alt\"><figure>fig</figure>",
    );
    let kinds: Vec<&BlockKind> = model.blocks.iter().map(|b| &b.kind).collect();
    assert_eq!(kinds[0], &BlockKind::Heading { level: 2 });
    assert_eq!(kinds[1], &BlockKind::Paragraph);
    assert_eq!(kinds[2], &BlockKind::Quote);
    assert_eq!(kinds[3], &BlockKind::Code { language: None });
    assert_eq!(kinds[4], &BlockKind::Divider);
    assert_eq!(
        kinds[5],
        &BlockKind::image("a.png".to_owned(), "Alt".to_owned())
    );
    assert!(matches!(kinds[6], BlockKind::Raw { source } if source.contains("<figure>")));
    assert_eq!(model.blocks[6].text, "");
}

#[test]
fn nested_lists_carry_depth_and_order() {
    let model = import("<ul><li>a<ol><li>b</li></ol></li><ul><li>c</li></ul></ul>");
    let items: Vec<(&str, &BlockKind)> = model
        .blocks
        .iter()
        .map(|b| (b.text.as_str(), &b.kind))
        .collect();
    assert_eq!(
        items,
        vec![
            ("a", &BlockKind::list_item(0, false)),
            ("b", &BlockKind::list_item(1, true)),
            ("c", &BlockKind::list_item(1, false)),
        ]
    );
}

#[test]
fn tables_import_cells_as_trimmed_paragraphs() {
    let model = import("<table><tr><th> H </th><td><b>x</b></td></tr><tr><td>y</td></tr></table>");
    let BlockKind::Table { rows, .. } = &model.blocks[0].kind else {
        panic!("not a table: {:?}", model.blocks[0].kind);
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].blocks[0].text, "H");
    assert_eq!(keys(&rows[0][1].blocks[0]), vec!["bold"]);
    assert_eq!(rows[1][0].blocks[0].text, "y");
}

#[test]
fn plain_containers_unwrap_and_attributed_ones_stay_raw() {
    let model = import("<div><p>in</p></div><div class=\"x\">kept</div>");
    assert_eq!(model.blocks[0].text, "in");
    assert!(matches!(&model.blocks[1].kind, BlockKind::Raw { source } if source.contains("kept")));
}

#[test]
fn loose_inline_text_becomes_a_paragraph_with_marks() {
    let model = import(
        "<em>it</em><a href=\"https://e.com\">l</a><br><mark>h</mark>\
         <mark style=\"background: blue\">b</mark><span style=\"color: red\">r</span>\
         <span data-comment=\"t1\">c</span><del>d</del><a>nohref</a>",
    );
    assert_eq!(model.blocks.len(), 1);
    let block = &model.blocks[0];
    assert_eq!(block.text, "itl\nhbrcdnohref");
    let kinds: Vec<&MarkKind> = block.marks.iter().map(|m| &m.kind).collect();
    assert!(kinds.contains(&&MarkKind::Italic));
    assert!(kinds.contains(&&MarkKind::Link {
        href: "https://e.com".to_owned()
    }));
    assert!(kinds.contains(&&MarkKind::Highlight {
        value: "yellow".to_owned()
    }));
    assert!(kinds.contains(&&MarkKind::Highlight {
        value: "blue".to_owned()
    }));
    assert!(kinds.contains(&&MarkKind::Color {
        value: "red".to_owned()
    }));
    assert!(kinds.contains(&&MarkKind::Comment {
        thread: "t1".to_owned()
    }));
    assert!(kinds.contains(&&MarkKind::Strike));
    assert_eq!(block.marks.len(), 7);
}

#[test]
fn loose_text_with_surrounding_space_drops_its_marks() {
    let model = import("  <b>x</b> tail  ");
    assert_eq!(model.blocks[0].text, "x tail");
    assert!(model.blocks[0].marks.is_empty());
}

#[test]
fn every_mark_renders_as_html_with_escaping() {
    let kinds = [
        MarkKind::Bold,
        MarkKind::Italic,
        MarkKind::Underline,
        MarkKind::Strike,
        MarkKind::Code,
        MarkKind::Link {
            href: "a\"b".to_owned(),
        },
        MarkKind::Color {
            value: "red".to_owned(),
        },
        MarkKind::Highlight {
            value: "yellow".to_owned(),
        },
        MarkKind::Comment {
            thread: "t".to_owned(),
        },
        MarkKind::Font {
            value: "Arial".to_owned(),
        },
        MarkKind::Size {
            value: "12".to_owned(),
        },
        MarkKind::Superscript,
        MarkKind::Subscript,
        MarkKind::Caps,
        MarkKind::Footnote {
            label: "n".to_owned(),
        },
        MarkKind::Field {
            code: "PAGE".to_owned(),
        },
    ];
    let mut block = Block::new("p", BlockKind::Paragraph, "<x>&\n");
    block.marks = kinds
        .into_iter()
        .map(|kind| Mark {
            start: 0,
            end: 5,
            kind,
        })
        .collect();
    let html = materialise(&RichText::of(vec![block]));
    for expected in [
        "&lt;x&gt;&amp;<br>",
        "<strong>",
        "<em>",
        "<u>",
        "<s>",
        "<code>",
        "<a href=\"a&quot;b\">",
        "color:red",
        "<mark style=\"background:yellow\">",
        "data-comment=\"t\"",
        "font-family:Arial",
        "font-size:12pt",
        "<sup>",
        "<sub>",
        "text-transform:uppercase",
        "data-footnote=\"n\"",
        "data-field=\"PAGE\"",
    ] {
        assert!(html.contains(expected), "{expected:?} in {html}");
    }
}

#[test]
fn segments_split_text_at_mark_boundaries() {
    let marks = vec![
        Mark {
            start: 0,
            end: 3,
            kind: MarkKind::Bold,
        },
        Mark {
            start: 2,
            end: 5,
            kind: MarkKind::Italic,
        },
    ];
    let runs: Vec<(String, Vec<&str>)> = segments("abcdef", &marks)
        .into_iter()
        .map(|(run, active)| (run, active.iter().map(|k| k.key()).collect()))
        .collect();
    assert_eq!(
        runs,
        vec![
            ("ab".to_owned(), vec!["bold"]),
            ("c".to_owned(), vec!["bold", "italic"]),
            ("de".to_owned(), vec!["italic"]),
            ("f".to_owned(), vec![]),
        ]
    );
}
