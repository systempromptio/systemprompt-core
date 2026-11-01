use systemprompt_files::documents::engine::loro::LoroEngine;
use systemprompt_files::documents::engine::loro::deck::{DeckModel, DeckSlide};
use systemprompt_files::documents::engine::loro::sheet::{Sheet, SheetCell, SheetModel};
use systemprompt_files::documents::engine::{DocumentEngine, EngineError, ImportScope, ModelView};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CellValue, ContentAs, EditOp, NodeTarget};

fn workbook(v: &str) -> ModelView {
    ModelView::Sheet(SheetModel {
        sheets: vec![Sheet::from_grid(
            "Data",
            [(
                1,
                1,
                SheetCell {
                    v: v.to_owned(),
                    ..SheetCell::default()
                },
            )],
            &[],
        )],
    })
}

fn cells(model: &ModelView) -> Vec<(u32, u32, String)> {
    let ModelView::Sheet(model) = model else {
        panic!("sheet");
    };
    let mut cells: Vec<_> = model.sheets[0]
        .grid()
        .into_iter()
        .map(|(col, row, cell)| (col, row, cell.v.clone()))
        .collect();
    cells.sort();
    cells
}

#[test]
fn engine_loro_a_scoped_sheet_import_reports_the_full_models() {
    let seed = workbook("1");
    let mut live = LoroEngine.seed(&seed).expect("seed");
    let update = live
        .update_to_model(&workbook("=A2+1"))
        .expect("diff")
        .expect("changed");
    let scoped = live.apply_update_scoped(&update, true).expect("apply");
    match scoped.scope {
        ImportScope::Full { before, after } => {
            assert_eq!(before, seed);
            assert_eq!(cells(&after), vec![(1, 1, "=A2+1".to_owned())]);
        },
        other => panic!("expected a full scope, got {other:?}"),
    }
}

#[test]
fn engine_loro_a_scoped_import_of_a_bad_formula_is_refused() {
    let mut live = LoroEngine.seed(&workbook("1")).expect("seed");
    let update = live
        .update_to_model(&workbook("=SUM("))
        .expect("diff")
        .expect("changed");
    assert!(matches!(
        live.apply_update_scoped(&update, true),
        Err(EngineError::Model(_))
    ));
}

#[test]
fn engine_loro_a_scoped_deck_import_with_a_duplicate_slide_is_refused() {
    let slide = |id: &str| DeckSlide {
        id: id.to_owned(),
        layout: None,
        shapes: Vec::new(),
    };
    let mut live = LoroEngine
        .seed(&ModelView::Deck(DeckModel {
            slides: vec![slide("s1")],
        }))
        .expect("seed");
    let update = live
        .update_to_model(&ModelView::Deck(DeckModel {
            slides: vec![slide("s1"), slide("s1")],
        }))
        .expect("diff")
        .expect("changed");
    assert!(matches!(
        live.apply_update_scoped(&update, true),
        Err(EngineError::Model(_))
    ));
}

#[test]
fn engine_loro_set_cells_ops_become_an_update_a_replica_applies() {
    let live = LoroEngine.seed(&workbook("1")).expect("seed");
    let mut replica = LoroEngine
        .open(&live.snapshot().expect("snapshot"))
        .expect("open");
    let ops = [EditOp::SetCells {
        sheet: "Data".to_owned(),
        range: "A1:B2".to_owned(),
        values: vec![
            vec![CellValue::Number(2.0), CellValue::Text("x".to_owned())],
            vec![CellValue::Empty, CellValue::Bool(true)],
        ],
        expected_hash: None,
    }];
    let out = live
        .update_from_ops(&ops, 4242)
        .expect("ops")
        .expect("changed");
    let import = replica.apply_update(&out.update).expect("apply");
    assert_eq!(import.peers, vec![4242]);
    let model = replica.export_model().expect("export");
    assert_eq!(model.clone(), {
        let mut held = live.fork();
        held.apply_update(&out.update).expect("apply");
        held.export_model().expect("export")
    });
    let values: Vec<String> = cells(&model).into_iter().map(|c| c.2).collect();
    assert_eq!(values.len(), 3);
    assert!(values.contains(&"x".to_owned()));
}

#[test]
fn engine_loro_unsupported_sheet_ops_answer_unsupported() {
    let live = LoroEngine.seed(&workbook("1")).expect("seed");
    let ops = [EditOp::AppendMarkdown {
        markdown: "# no".to_owned(),
    }];
    assert!(matches!(
        live.update_from_ops(&ops, 7),
        Err(EngineError::Unsupported)
    ));
    let bad = [EditOp::SetCells {
        sheet: "Data".to_owned(),
        range: "A1:B1".to_owned(),
        values: vec![vec![CellValue::Empty]],
        expected_hash: None,
    }];
    assert!(matches!(
        live.update_from_ops(&bad, 7),
        Err(EngineError::Model(_))
    ));
}

#[test]
fn engine_loro_rich_ops_rewrite_the_named_block() {
    let live = LoroEngine
        .seed(&ModelView::RichText(RichText::of(vec![
            Block::new("b1", BlockKind::Paragraph, "old"),
            Block::new("b2", BlockKind::Paragraph, "keep"),
        ])))
        .expect("seed");
    let ops = [EditOp::ReplaceText {
        node: NodeTarget::Id(DocumentNodeId::new("b1")),
        expected_hash: None,
        text: "new".to_owned(),
        content_as: ContentAs::Text,
    }];
    let out = live
        .update_from_ops(&ops, 9)
        .expect("ops")
        .expect("changed");
    let mut replica = live.fork();
    replica.apply_update(&out.update).expect("apply");
    let ModelView::RichText(model) = replica.export_model().expect("export") else {
        panic!("rich text");
    };
    let texts: Vec<&str> = model.blocks.iter().map(|b| b.text.as_str()).collect();
    assert_eq!(texts, vec!["new", "keep"]);
    let unknown = [EditOp::ReplaceText {
        node: NodeTarget::Id(DocumentNodeId::new("missing")),
        expected_hash: None,
        text: "x".to_owned(),
        content_as: ContentAs::Text,
    }];
    assert!(matches!(
        live.update_from_ops(&unknown, 9),
        Err(EngineError::Unsupported)
    ));
}

#[test]
fn engine_loro_an_insert_slide_op_adds_a_slide() {
    let live = LoroEngine
        .seed(&ModelView::Deck(DeckModel {
            slides: vec![DeckSlide {
                id: "s1".to_owned(),
                layout: None,
                shapes: Vec::new(),
            }],
        }))
        .expect("seed");
    let ops = [EditOp::InsertSlide {
        after: None,
        layout: None,
        title: Some("Added".to_owned()),
        body: vec!["point".to_owned()],
    }];
    let out = live
        .update_from_ops(&ops, 11)
        .expect("ops")
        .expect("changed");
    let mut replica = live.fork();
    replica.apply_update(&out.update).expect("apply");
    let ModelView::Deck(deck) = replica.export_model().expect("export") else {
        panic!("deck");
    };
    assert_eq!(deck.slides.len(), 2);
    let texts: Vec<String> = deck
        .slides
        .iter()
        .flat_map(|s| s.shapes.iter())
        .flat_map(|s| s.paragraphs.iter())
        .map(|p| p.text.clone())
        .collect();
    assert!(texts.contains(&"Added".to_owned()));
}
