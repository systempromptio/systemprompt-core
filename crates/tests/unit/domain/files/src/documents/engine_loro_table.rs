use loro::{ExportMode, LoroDoc};
use serde_json::json;
use systemprompt_files::documents::engine::loro::edit::{
    cell_blocks, delete_column, delete_row, insert_column, insert_row, set_cell_prop,
    set_table_prop, set_text,
};
use systemprompt_files::documents::engine::loro::table::canonical_table;
use systemprompt_files::documents::engine::loro::{rich, seed_rich, write_model};
use systemprompt_files::documents::model::layout::{Align, CellProps, TableProps};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};

fn cell(id: &str, text: &str) -> RichText {
    RichText::of(vec![Block::new(id, BlockKind::Paragraph, text)])
}

fn table_model(props: TableProps) -> RichText {
    RichText::of(vec![Block::new(
        "t",
        BlockKind::Table {
            rows: vec![
                vec![cell("c00", "a"), cell("c01", "b")],
                vec![cell("c10", "c"), cell("c11", "d")],
            ],
            props,
        },
        "",
    )])
}

fn table_of(doc: &LoroDoc) -> (Vec<Vec<String>>, TableProps) {
    let model = rich::read(doc).expect("read");
    let BlockKind::Table { rows, props } = model.blocks[0].kind.clone() else {
        panic!("table");
    };
    let texts = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| cell.blocks.iter().map(|b| b.text.as_str()).collect())
                .collect()
        })
        .collect();
    (texts, props)
}

fn styled() -> TableProps {
    TableProps {
        header_rows: 1,
        style: Some("Grid".to_owned()),
        cells: vec![vec![CellProps {
            shade: Some("FF0000".to_owned()),
            align: Some(Align::Center),
            colspan: 2,
            ..CellProps::default()
        }]],
        ..TableProps::default()
    }
}

#[test]
fn engine_loro_a_styled_table_round_trips_through_containers() {
    let model = table_model(styled());
    let doc = seed_rich(&model).expect("seed");
    assert_eq!(rich::read(&doc).expect("read"), model);
}

#[test]
fn engine_loro_canonical_table_drops_default_styling() {
    let mut rows = vec![vec![cell("a", "a"), cell("b", "b")]];
    let mut props = TableProps {
        cells: vec![
            vec![
                CellProps {
                    colspan: 1,
                    rowspan: 1,
                    ..CellProps::default()
                },
                CellProps::default(),
            ],
            vec![CellProps::default()],
        ],
        ..TableProps::default()
    };
    canonical_table(&mut rows, &mut props);
    assert!(props.cells.is_empty());
}

#[test]
fn engine_loro_rewriting_a_table_model_replaces_cells_and_props() {
    let doc = seed_rich(&table_model(styled())).expect("seed");
    let next = RichText::of(vec![Block::new(
        "t",
        BlockKind::Table {
            rows: vec![vec![cell("c00", "only")]],
            props: TableProps::default(),
        },
        "",
    )]);
    write_model(&doc, &next).expect("write");
    doc.commit();
    assert_eq!(rich::read(&doc).expect("read"), next);
}

#[test]
fn engine_loro_rows_and_columns_are_inserted_and_deleted_by_id() {
    let doc = seed_rich(&table_model(TableProps::default())).expect("seed");
    let (row_id, cell_ids) = insert_row(&doc, "t", 1, 2).expect("row");
    assert_eq!(cell_ids.len(), 2);
    let added = cell_blocks(&doc, &cell_ids[1]).expect("cell blocks");
    assert_eq!(added.len(), 1);
    let column = insert_column(&doc, "t", 0).expect("column");
    assert_eq!(column.len(), 3);
    doc.commit();
    let (texts, _) = table_of(&doc);
    assert_eq!(
        texts,
        vec![vec!["", "a", "b"], vec!["", "", ""], vec!["", "c", "d"],]
    );
    delete_row(&doc, &row_id).expect("delete row");
    delete_column(&doc, "t", 0).expect("delete column");
    delete_column(&doc, "t", 9).expect("past the end is a no-op");
    doc.commit();
    assert_eq!(table_of(&doc).0, vec![vec!["a", "b"], vec!["c", "d"]]);
}

#[test]
fn engine_loro_table_and_cell_props_are_set_and_cleared() {
    let doc = seed_rich(&table_model(TableProps::default())).expect("seed");
    let (_, ids) = insert_row(&doc, "t", 0, 1).expect("row");
    set_table_prop(&doc, "t", "header_rows", Some(&json!(1))).expect("header");
    set_table_prop(&doc, "t", "style", Some(&json!("Plain"))).expect("style");
    set_cell_prop(&doc, &ids[0], "shade", Some(&json!("00FF00"))).expect("shade");
    doc.commit();
    let (_, props) = table_of(&doc);
    assert_eq!(props.header_rows, 1);
    assert_eq!(props.style.as_deref(), Some("Plain"));
    assert_eq!(props.cells[0][0].shade.as_deref(), Some("00FF00"));
    set_table_prop(&doc, "t", "style", None).expect("clear");
    set_table_prop(&doc, "t", "style", None).expect("clear twice");
    set_cell_prop(&doc, &ids[0], "shade", None).expect("clear shade");
    doc.commit();
    let (_, props) = table_of(&doc);
    assert_eq!(props.style, None);
    assert!(props.cells.is_empty());
    assert!(set_cell_prop(&doc, "missing", "shade", None).is_err());
}

#[test]
fn engine_loro_text_in_a_cell_is_found_and_edited_by_block_id() {
    let doc = seed_rich(&table_model(TableProps::default())).expect("seed");
    set_text(&doc, "c11", "dd").expect("cell text");
    doc.commit();
    assert_eq!(table_of(&doc).0[1][1], "dd");
}

#[test]
fn engine_loro_concurrent_cell_edits_and_row_inserts_converge() {
    let left = seed_rich(&table_model(TableProps::default())).expect("seed");
    let right = LoroDoc::new();
    rich::configure(&right);
    right
        .import(&left.export(ExportMode::Snapshot).expect("snapshot"))
        .expect("import");
    right.set_peer_id(91).expect("peer");
    let base = left.oplog_vv();
    set_text(&left, "c00", "A").expect("left cell");
    set_table_prop(&left, "t", "header_rows", Some(&json!(1))).expect("left prop");
    left.commit();
    set_text(&right, "c11", "D").expect("right cell");
    insert_row(&right, "t", 2, 2).expect("right row");
    right.commit();
    let from_left = left.export(ExportMode::updates(&base)).expect("left");
    let from_right = right.export(ExportMode::updates(&base)).expect("right");
    left.import(&from_right).expect("into left");
    right.import(&from_left).expect("into right");
    assert_eq!(table_of(&left), table_of(&right));
    let (texts, props) = table_of(&left);
    assert_eq!(texts, vec![vec!["A", "b"], vec!["c", "D"], vec!["", ""]]);
    assert_eq!(props.header_rows, 1);
}
