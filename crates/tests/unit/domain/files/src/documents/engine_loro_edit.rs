use loro::{ExportMode, LoroDoc};
use systemprompt_files::documents::engine::loro::edit::{
    self, EditError, delete_block, delete_text, find_block, insert_block_near, insert_root_block,
    insert_text, mark, move_block, set_kind, set_text, unmark,
};
use systemprompt_files::documents::engine::loro::{rich, seed_rich};
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind, RichText};

fn doc_of(texts: &[&str]) -> LoroDoc {
    seed_rich(&RichText::of(
        texts
            .iter()
            .enumerate()
            .map(|(n, text)| Block::new(format!("b{n}"), BlockKind::Paragraph, *text))
            .collect(),
    ))
    .expect("seed")
}

fn texts(doc: &LoroDoc) -> Vec<(String, String)> {
    rich::read(doc)
        .expect("read")
        .blocks
        .into_iter()
        .map(|block| (block.id, block.text))
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(id, text)| ((*id).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn engine_loro_text_edits_land_on_the_named_block() {
    let doc = doc_of(&["alpha", "beta"]);
    set_text(&doc, "b0", "alpha one").expect("set");
    insert_text(&doc, "b1", 0, "the ").expect("insert");
    delete_text(&doc, "b1", 4, 1).expect("delete");
    doc.commit();
    assert_eq!(
        texts(&doc),
        pairs(&[("b0", "alpha one"), ("b1", "the eta")])
    );
}

#[test]
fn engine_loro_marks_are_added_and_removed_by_range() {
    let doc = doc_of(&["bold link"]);
    mark(&doc, "b0", 0..4, &MarkKind::Bold).expect("bold");
    let link = MarkKind::Link {
        href: "https://example.com".to_owned(),
    };
    mark(&doc, "b0", 5..9, &link).expect("link");
    doc.commit();
    let marks = rich::read(&doc).expect("read").blocks[0].marks.clone();
    assert!(marks.contains(&Mark {
        start: 0,
        end: 4,
        kind: MarkKind::Bold
    }));
    assert!(marks.contains(&Mark {
        start: 5,
        end: 9,
        kind: link
    }));
    unmark(&doc, "b0", 0..4, MarkKind::Bold.key()).expect("unmark");
    doc.commit();
    let marks = rich::read(&doc).expect("read").blocks[0].marks.clone();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].start, 5);
}

#[test]
fn engine_loro_set_kind_rewrites_the_kind_and_keeps_the_text() {
    let doc = doc_of(&["title"]);
    set_kind(&doc, &Block::new("b0", BlockKind::Heading { level: 2 }, "")).expect("kind");
    doc.commit();
    let block = &rich::read(&doc).expect("read").blocks[0];
    assert_eq!(block.kind, BlockKind::Heading { level: 2 });
    assert_eq!(block.text, "title");
}

#[test]
fn engine_loro_blocks_are_inserted_moved_and_deleted_by_id() {
    let doc = doc_of(&["a", "b", "c"]);
    insert_block_near(
        &doc,
        "b1",
        true,
        &Block::new("x", BlockKind::Paragraph, "x"),
    )
    .expect("before");
    insert_block_near(
        &doc,
        "b1",
        false,
        &Block::new("y", BlockKind::Paragraph, "y"),
    )
    .expect("after");
    insert_root_block(&doc, 99, &Block::new("z", BlockKind::Quote, "z")).expect("root");
    doc.commit();
    assert_eq!(
        texts(&doc),
        pairs(&[
            ("b0", "a"),
            ("x", "x"),
            ("b1", "b"),
            ("y", "y"),
            ("b2", "c"),
            ("z", "z")
        ])
    );
    move_block(&doc, "z", 0).expect("move");
    move_block(&doc, "b0", 99).expect("move to end");
    delete_block(&doc, "x").expect("delete");
    doc.commit();
    assert_eq!(
        texts(&doc),
        pairs(&[
            ("z", "z"),
            ("b1", "b"),
            ("y", "y"),
            ("b2", "c"),
            ("b0", "a")
        ])
    );
    assert_eq!(find_block(&doc, "y").expect("found").at, 2);
}

#[test]
fn engine_loro_edits_on_unknown_ids_are_refused_as_not_found() {
    let doc = doc_of(&["a"]);
    for result in [
        set_text(&doc, "nope", "x"),
        delete_block(&doc, "nope"),
        move_block(&doc, "nope", 0),
        edit::delete_row(&doc, "nope"),
        edit::set_table_prop(&doc, "nope", "style", None),
    ] {
        match result {
            Err(EditError::NotFound { id }) => assert_eq!(id, "nope"),
            other => panic!("expected not found, got {other:?}"),
        }
    }
    match edit::insert_row(&doc, "b0", 0, 1) {
        Err(error @ EditError::NotATable { .. }) => {
            assert_eq!(error.to_string(), "block b0 is not a table");
        },
        other => panic!("expected not a table, got {other:?}"),
    }
}

#[test]
fn engine_loro_concurrent_block_edits_converge() {
    let left = doc_of(&["one", "two", "three"]);
    let right = LoroDoc::new();
    rich::configure(&right);
    right
        .import(&left.export(ExportMode::Snapshot).expect("snapshot"))
        .expect("import");
    right.set_peer_id(77).expect("peer");
    let base = left.oplog_vv();
    insert_text(&left, "b0", 3, "!").expect("left text");
    move_block(&left, "b2", 0).expect("left move");
    left.commit();
    set_text(&right, "b1", "TWO").expect("right text");
    insert_root_block(&right, 3, &Block::new("r", BlockKind::Paragraph, "four"))
        .expect("right insert");
    right.commit();
    let from_left = left.export(ExportMode::updates(&base)).expect("left");
    let from_right = right.export(ExportMode::updates(&base)).expect("right");
    left.import(&from_right).expect("into left");
    right.import(&from_left).expect("into right");
    assert_eq!(texts(&left), texts(&right));
    assert_eq!(
        texts(&left),
        pairs(&[
            ("b2", "three"),
            ("b0", "one!"),
            ("b1", "TWO"),
            ("r", "four")
        ])
    );
}
