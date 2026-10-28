use systemprompt_files::documents::model::{
    Block, BlockKind, EditLevel, Limits, Mark, MarkKind, ModelRejection, RichText, carry_ids, html,
    markdown, validate,
};

fn para(id: &str, text: &str) -> Block {
    Block::new(id, BlockKind::Paragraph, text)
}

fn marked(mut block: Block, start: usize, end: usize, kind: MarkKind) -> Block {
    block.marks.push(Mark { start, end, kind });
    block
}

fn check(before: &RichText, after: &RichText, level: EditLevel) -> Result<(), ModelRejection> {
    validate(before, after, level, &Limits::default())
}

#[test]
fn markdown_imports_blocks_and_marks_with_deterministic_ids() {
    let model = markdown::import(
        "# Title\n\nSome **bold** and [a link](https://x.io).\n\n- one\n  - two\n",
    );
    let kinds: Vec<_> = model.blocks.iter().map(|b| b.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            BlockKind::Heading { level: 1 },
            BlockKind::Paragraph,
            BlockKind::ListItem {
                level: 0,
                ordered: false,
                numbering: None,
                checked: None,
            },
            BlockKind::ListItem {
                level: 1,
                ordered: false,
                numbering: None,
                checked: None,
            },
        ]
    );
    let ids: Vec<_> = model.blocks.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["b1", "b2", "b3", "b4"]);
    let body = &model.blocks[1];
    assert_eq!(body.text, "Some bold and a link.");
    assert!(body.marks.contains(&Mark {
        start: 5,
        end: 9,
        kind: MarkKind::Bold
    }));
    assert!(body.marks.iter().any(|m| m.kind
        == MarkKind::Link {
            href: "https://x.io".into()
        }));
}

#[test]
fn markdown_materialises_and_reimports_to_the_same_model() {
    let model = markdown::import(
        "## Plan\n\nA *small* `step`.\n\n1. first\n\n> quoted\n\n```rust\nfn x() {}\n```\n\n---\n",
    );
    let again = markdown::import(&markdown::materialise(&model).expect("materialise"));
    assert_eq!(again, model);
}

#[test]
fn html_round_trips_blocks_and_keeps_unknown_markup_verbatim() {
    let body = "<h2>Hi</h2><p>An <strong>ok</strong> <u>day</u></p><div class=\"card\"><b>x</b></div><ul><li>a<ul><li>b</li></ul></li></ul>";
    let model = html::import(body);
    assert!(model.blocks.iter().any(|b| b.kind
        == BlockKind::Raw {
            source: "<div class=\"card\"><b>x</b></div>".into()
        }));
    assert!(model.blocks.iter().any(|b| b.kind
        == BlockKind::ListItem {
            level: 1,
            ordered: false,
            numbering: None,
            checked: None,
        }));
    let again = html::import(&html::materialise(&model));
    assert_eq!(again, model);
}

#[test]
fn a_valid_edit_passes_and_a_disallowed_link_is_rejected() {
    let before = RichText {
        blocks: vec![para("b1", "hello")],
        document: None,
    };
    let ok = RichText {
        blocks: vec![marked(para("b1", "hello"), 0, 5, MarkKind::Bold)],
        document: None,
    };
    assert!(check(&before, &ok, EditLevel::Edit).is_ok());
    let evil = RichText {
        blocks: vec![marked(
            para("b1", "hello"),
            0,
            5,
            MarkKind::Link {
                href: "javascript:alert(1)".into(),
            },
        )],
        document: None,
    };
    assert_eq!(
        check(&before, &evil, EditLevel::Edit).map_err(|e| e.reason()),
        Err("bad_mark")
    );
}

#[test]
fn duplicate_ids_marks_past_the_text_and_oversize_models_are_rejected() {
    let empty = RichText::default();
    let twice = RichText {
        blocks: vec![para("b1", "a"), para("b1", "b")],
        document: None,
    };
    assert_eq!(
        check(&empty, &twice, EditLevel::Edit).map_err(|e| e.reason()),
        Err("bad_block_id")
    );
    let past = RichText {
        blocks: vec![marked(para("b1", "ab"), 0, 9, MarkKind::Italic)],
        document: None,
    };
    assert_eq!(
        check(&empty, &past, EditLevel::Edit).map_err(|e| e.reason()),
        Err("bad_mark")
    );
    let limits = Limits {
        blocks: 1,
        ..Limits::default()
    };
    let many = RichText {
        blocks: vec![para("b1", "a"), para("b2", "b")],
        document: None,
    };
    assert_eq!(
        validate(&empty, &many, EditLevel::Edit, &limits).map_err(|e| e.reason()),
        Err("too_large")
    );
}

#[test]
fn a_live_update_cannot_author_raw_markup() {
    let empty = RichText::default();
    let raw = RichText {
        blocks: vec![Block::new(
            "x",
            BlockKind::Raw {
                source: "<script>1</script>".into(),
            },
            "",
        )],
        document: None,
    };
    assert_eq!(
        check(&empty, &raw, EditLevel::Edit).map_err(|e| e.reason()),
        Err("raw_block_changed")
    );
    assert!(check(&raw, &raw, EditLevel::Edit).is_ok());
}

#[test]
fn a_comment_level_caller_may_only_change_comment_marks() {
    let before = RichText {
        blocks: vec![para("b1", "hello")],
        document: None,
    };
    let commented = RichText {
        blocks: vec![marked(
            para("b1", "hello"),
            1,
            3,
            MarkKind::Comment {
                thread: "t1".into(),
            },
        )],
        document: None,
    };
    assert!(check(&before, &commented, EditLevel::Comment).is_ok());
    let rewritten = RichText {
        blocks: vec![para("b1", "goodbye")],
        document: None,
    };
    assert_eq!(
        check(&before, &rewritten, EditLevel::Comment),
        Err(ModelRejection::BeyondCommentLevel)
    );
}

#[test]
fn a_reimport_carries_the_live_ids_and_never_reuses_one() {
    let live = RichText {
        blocks: vec![para("u.1", "one"), para("u.2", "two"), para("u.3", "three")],
        document: None,
    };
    let mut next = RichText {
        blocks: vec![
            para("b1", "one"),
            para("b2", "two!"),
            para("b3", "new"),
            para("b4", "three"),
        ],
        document: None,
    };
    carry_ids(&live, &mut next);
    let ids: Vec<_> = next.blocks.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids[0], "u.1");
    assert_eq!(ids[1], "u.2");
    assert_eq!(ids[3], "u.3");
    assert!(!["u.1", "u.2", "u.3"].contains(&ids[2]));
}
