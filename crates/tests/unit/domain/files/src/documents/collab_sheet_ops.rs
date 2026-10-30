use systemprompt_files::documents::engine::loro::LoroEngine;
use systemprompt_files::documents::engine::loro::sheet::model::{Sheet, SheetCell, SheetModel};
use systemprompt_files::documents::engine::{DocumentEngine, EngineDoc, ModelView};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CellValue, ContentAs, EditOp, NodeKind, NodeTarget,
};

const PEER: u64 = 4242;

fn cell(v: &str) -> SheetCell {
    SheetCell {
        v: v.to_owned(),
        ..SheetCell::default()
    }
}

fn seeded() -> Box<dyn EngineDoc> {
    let sheet = Sheet::from_grid("Data", [(1, 1, cell("head")), (2, 2, cell("old"))], &[]);
    LoroEngine
        .seed(&ModelView::Sheet(SheetModel {
            sheets: vec![sheet],
        }))
        .expect("seed")
}

fn applied(ops: &[EditOp]) -> SheetModel {
    let doc = seeded();
    let mut replica = doc.fork();
    let update = doc
        .update_from_ops(ops, PEER)
        .expect("translate")
        .expect("changed");
    replica.apply_update(&update.update).expect("import");
    match replica.export_model().expect("export") {
        ModelView::Sheet(model) => model,
        other => panic!("not a sheet: {other:?}"),
    }
}

fn value(model: &SheetModel, col: u32, row: u32) -> Option<String> {
    let sheet = model.sheet("Data")?;
    let (r, c) = sheet.ids_at(col, row)?;
    sheet
        .cells
        .get(&format!("{r}:{c}"))
        .map(|cell| cell.v.clone())
}

fn node(id: &str) -> NodeTarget {
    NodeTarget::Id(DocumentNodeId::new(id))
}

fn text(value: &str) -> CellValue {
    CellValue::Text(value.to_owned())
}

#[test]
fn set_cells_writes_each_typed_value_of_the_range() {
    let model = applied(&[EditOp::SetCells {
        sheet: "Data".to_owned(),
        range: "A2:C2".to_owned(),
        values: vec![vec![
            CellValue::Number(2.5),
            CellValue::Bool(true),
            CellValue::Bool(false),
        ]],
        expected_hash: None,
    }]);
    assert_eq!(value(&model, 1, 2).as_deref(), Some("2.5"));
    assert_eq!(value(&model, 2, 2).as_deref(), Some("TRUE"));
    assert_eq!(value(&model, 3, 2).as_deref(), Some("FALSE"));
    assert_eq!(value(&model, 1, 1).as_deref(), Some("head"));
}

#[test]
fn replace_text_on_a_cell_node_writes_that_cell() {
    let model = applied(&[EditOp::ReplaceText {
        node: node("sheet[Data]/B2"),
        expected_hash: None,
        text: "new".to_owned(),
        content_as: ContentAs::Text,
    }]);
    assert_eq!(value(&model, 2, 2).as_deref(), Some("new"));
}

#[test]
fn insert_after_a_row_spreads_tab_separated_text_across_a_new_row() {
    let model = applied(&[EditOp::InsertAfter {
        node: node("sheet[Data]/row[1]"),
        kind: NodeKind::Row,
        text: "x\t\tz".to_owned(),
        content_as: ContentAs::Text,
    }]);
    assert_eq!(value(&model, 1, 2).as_deref(), Some("x"));
    assert_eq!(value(&model, 2, 2), None);
    assert_eq!(value(&model, 3, 2).as_deref(), Some("z"));
    assert_eq!(value(&model, 2, 3).as_deref(), Some("old"));
}

#[test]
fn insert_after_the_sheet_node_prepends_a_row() {
    let model = applied(&[EditOp::InsertAfter {
        node: node("sheet[Data]"),
        kind: NodeKind::Row,
        text: "top".to_owned(),
        content_as: ContentAs::Text,
    }]);
    assert_eq!(value(&model, 1, 1).as_deref(), Some("top"));
    assert_eq!(value(&model, 1, 2).as_deref(), Some("head"));
}

#[test]
fn set_cells_overwrites_an_existing_cell() {
    let model = applied(&[EditOp::SetCells {
        sheet: "Data".to_owned(),
        range: "A1".to_owned(),
        values: vec![vec![text("first")]],
        expected_hash: None,
    }]);
    assert_eq!(value(&model, 1, 1).as_deref(), Some("first"));
}

#[test]
fn refused_batches_write_nothing() {
    let doc = seeded();
    let before = doc.version();
    for ops in [
        vec![EditOp::SetCells {
            sheet: "Data".to_owned(),
            range: "A1:B1".to_owned(),
            values: vec![vec![text("only one")]],
            expected_hash: None,
        }],
        vec![EditOp::SetCells {
            sheet: "Data".to_owned(),
            range: "!!".to_owned(),
            values: Vec::new(),
            expected_hash: None,
        }],
        vec![EditOp::ReplaceText {
            node: node("sheet[Data]/row[1]"),
            expected_hash: None,
            text: "x".to_owned(),
            content_as: ContentAs::Text,
        }],
        vec![EditOp::InsertAfter {
            node: node("sheet[Data]/A1"),
            kind: NodeKind::Row,
            text: "x".to_owned(),
            content_as: ContentAs::Text,
        }],
        vec![EditOp::StrReplace {
            old_str: "old".to_owned(),
            new_str: "x".to_owned(),
        }],
        vec![
            EditOp::SetCells {
                sheet: "Data".to_owned(),
                range: "A1".to_owned(),
                values: vec![vec![text("kept out")]],
                expected_hash: None,
            },
            EditOp::SetCells {
                sheet: "Missing".to_owned(),
                range: "A1".to_owned(),
                values: vec![vec![text("x")]],
                expected_hash: None,
            },
        ],
    ] {
        assert!(doc.update_from_ops(&ops, PEER).is_err(), "{ops:?}");
        assert_eq!(doc.version(), before);
    }
}
