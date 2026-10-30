use std::num::NonZeroUsize;

use systemprompt_files::documents::engine::loro::LoroEngine;
use systemprompt_files::documents::engine::{DocumentEngine, EngineDoc, EngineError, ModelView};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    ContentAs, EditOp, NodeKind, NodeTarget, SetBlockType, SetMarks, TableDelete, TableInsert,
    TextBlockType, hash8,
};

const PEER: u64 = 9001;

fn model() -> RichText {
    let mut model = RichText::default();
    model.blocks = vec![
        Block::new("h1", BlockKind::Heading { level: 1 }, "Title"),
        Block::new("p1", BlockKind::Paragraph, "first paragraph"),
        Block::new("p2", BlockKind::Paragraph, "second paragraph"),
    ];
    model
}

fn seeded() -> Box<dyn EngineDoc> {
    LoroEngine
        .seed(&ModelView::RichText(model()))
        .expect("seed")
}

fn applied(ops: &[EditOp]) -> (RichText, Vec<String>) {
    let doc = seeded();
    let mut replica = doc.fork();
    let update = doc
        .update_from_ops(ops, PEER)
        .expect("translate")
        .expect("changed");
    replica.apply_update(&update.update).expect("import");
    match replica.export_model().expect("export") {
        ModelView::RichText(model) => (model, update.touched),
        other => panic!("not rich text: {other:?}"),
    }
}

fn texts(model: &RichText) -> Vec<&str> {
    model.blocks.iter().map(|b| b.text.as_str()).collect()
}

fn node(id: &str) -> NodeTarget {
    NodeTarget::Id(DocumentNodeId::new(id))
}

#[test]
fn replace_text_checks_the_live_hash_and_names_the_block() {
    let (model, touched) = applied(&[EditOp::ReplaceText {
        node: node("p1"),
        expected_hash: Some(hash8("first paragraph")),
        text: "rewritten".to_owned(),
        content_as: ContentAs::Text,
    }]);
    assert_eq!(texts(&model), ["Title", "rewritten", "second paragraph"]);
    assert_eq!(touched, ["p1"]);
}

#[test]
fn insert_after_and_delete_reshape_the_block_list() {
    let (model, touched) = applied(&[
        EditOp::InsertAfter {
            node: node("h1"),
            kind: NodeKind::Heading { level: 2 },
            text: "Sub".to_owned(),
            content_as: ContentAs::Text,
        },
        EditOp::Delete {
            node: node("p2"),
            expected_hash: None,
        },
    ]);
    assert_eq!(texts(&model), ["Title", "Sub", "first paragraph"]);
    assert_eq!(model.blocks[1].kind, BlockKind::Heading { level: 2 });
    assert_eq!(touched, [model.blocks[1].id.clone()]);
}

#[test]
fn str_replace_edits_the_one_block_holding_the_quote() {
    let (model, _) = applied(&[EditOp::StrReplace {
        old_str: "second".to_owned(),
        new_str: "last".to_owned(),
    }]);
    assert_eq!(
        texts(&model),
        ["Title", "first paragraph", "last paragraph"]
    );
}

#[test]
fn set_block_type_changes_the_kind_and_keeps_the_text() {
    let (model, _) = applied(&[
        EditOp::SetBlockType(SetBlockType {
            node: node("p1"),
            expected_hash: None,
            block: TextBlockType::Quote,
        }),
        EditOp::SetBlockType(SetBlockType {
            node: node("p2"),
            expected_hash: None,
            block: TextBlockType::ListItem {
                level: 1,
                ordered: true,
            },
        }),
    ]);
    assert_eq!(model.blocks[1].kind, BlockKind::Quote);
    assert_eq!(model.blocks[2].kind, BlockKind::list_item(1, true));
    assert_eq!(
        texts(&model),
        ["Title", "first paragraph", "second paragraph"]
    );
}

#[test]
fn set_marks_removing_an_absent_mark_changes_nothing() {
    let doc = seeded();
    let update = doc
        .update_from_ops(
            &[EditOp::SetMarks(SetMarks {
                node: node("p1"),
                expected_hash: None,
                find: Some("paragraph".to_owned()),
                nth: NonZeroUsize::new(1),
                add: Vec::new(),
                remove: vec!["bold".to_owned()],
            })],
            PEER,
        )
        .expect("translate");
    if let Some(update) = update {
        let mut replica = doc.fork();
        replica.apply_update(&update.update).expect("import");
        let ModelView::RichText(after) = replica.export_model().expect("export") else {
            panic!("not rich text");
        };
        assert_eq!(texts(&after), texts(&model()));
    }
}

#[test]
fn table_rows_are_inserted_and_deleted() {
    let mut seed = model();
    let cell = |text: &str| {
        let mut cell = RichText::default();
        cell.blocks = vec![Block::new(format!("c-{text}"), BlockKind::Paragraph, text)];
        cell
    };
    let mut table = Block::new(
        "t1",
        BlockKind::table(vec![
            vec![cell("a1"), cell("b1")],
            vec![cell("a2"), cell("b2")],
        ]),
        "",
    );
    table.text = String::new();
    seed.blocks.push(table);
    let doc = LoroEngine.seed(&ModelView::RichText(seed)).expect("seed");
    let rows = |doc: &dyn EngineDoc| match doc.export_model().expect("export") {
        ModelView::RichText(model) => match &model.blocks[3].kind {
            BlockKind::Table { rows, .. } => rows.len(),
            other => panic!("not a table: {other:?}"),
        },
        other => panic!("not rich text: {other:?}"),
    };
    let mut replica = doc.fork();
    let inserted = doc
        .update_from_ops(
            &[EditOp::TableInsertRow(TableInsert {
                table: node("t1"),
                expected_hash: None,
                at: None,
                cells: vec!["a3".to_owned(), "b3".to_owned()],
            })],
            PEER,
        )
        .expect("insert")
        .expect("changed");
    assert_eq!(inserted.touched.len(), 2);
    replica.apply_update(&inserted.update).expect("import");
    assert_eq!(rows(replica.as_ref()), 3);
    let deleted = replica
        .update_from_ops(
            &[EditOp::TableDelete(TableDelete {
                table: node("t1"),
                expected_hash: None,
                row: Some(0),
                col: None,
            })],
            PEER + 1,
        )
        .expect("delete")
        .expect("changed");
    replica.apply_update(&deleted.update).expect("import");
    assert_eq!(rows(replica.as_ref()), 2);
}

#[test]
fn guards_unknown_blocks_and_untranslated_ops_refuse_the_whole_batch() {
    let doc = seeded();
    let before = doc.version();
    let refusals = [
        vec![EditOp::ReplaceText {
            node: node("p1"),
            expected_hash: Some("deadbeef".to_owned()),
            text: "x".to_owned(),
            content_as: ContentAs::Text,
        }],
        vec![EditOp::Delete {
            node: node("missing"),
            expected_hash: None,
        }],
        vec![EditOp::StrReplace {
            old_str: "paragraph".to_owned(),
            new_str: "x".to_owned(),
        }],
        vec![EditOp::AppendMarkdown {
            markdown: "# x".to_owned(),
        }],
        vec![EditOp::InsertAfter {
            node: node("p1"),
            kind: NodeKind::Paragraph,
            text: "**md**".to_owned(),
            content_as: ContentAs::Markdown,
        }],
        vec![
            EditOp::ReplaceText {
                node: node("p1"),
                expected_hash: None,
                text: "kept out".to_owned(),
                content_as: ContentAs::Text,
            },
            EditOp::Delete {
                node: node("missing"),
                expected_hash: None,
            },
        ],
    ];
    for ops in refusals {
        let refused = doc.update_from_ops(&ops, PEER);
        assert!(refused.is_err(), "{ops:?}");
        assert_eq!(doc.version(), before);
    }
    assert!(matches!(
        doc.update_from_ops(
            &[EditOp::SetText {
                text: "x".to_owned()
            }],
            PEER
        ),
        Err(EngineError::Unsupported)
    ));
}
