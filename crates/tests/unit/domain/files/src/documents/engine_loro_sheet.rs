use loro::{ExportMode, LoroDoc};
use systemprompt_files::documents::engine::loro::sheet::a1::{
    A1Range, SheetTarget, a1, column_letters, parse_a1, parse_node,
};
use systemprompt_files::documents::engine::loro::sheet::edit::{
    ensure_cell_key, insert_row_after, row_number, set_cell_input, sheet_map,
};
use systemprompt_files::documents::engine::loro::sheet::{
    self, Sheet, SheetCell, SheetChange, SheetModel, SheetValidationError, seed_sheets,
    validate_sheet_changes, write_sheets,
};
use systemprompt_files::documents::engine::loro::{LoroEngine, rich, seed_model};
use systemprompt_files::documents::engine::{DocumentEngine, ModelView};

fn input(v: &str) -> SheetCell {
    SheetCell {
        v: v.to_owned(),
        ..SheetCell::default()
    }
}

fn workbook() -> SheetModel {
    let mut styled = input("=A1*2");
    styled.fmt = Some("0.00".to_owned());
    styled.style = Some("{\"font\":{\"b\":true}}".to_owned());
    SheetModel {
        sheets: vec![Sheet::from_grid(
            "Data",
            [(1, 1, input("2")), (2, 3, styled)],
            &["A4:B5".to_owned()],
        )],
    }
}

fn a1_cells(model: &SheetModel, name: &str) -> Vec<(String, String)> {
    let sheet = model.sheet(name).expect("sheet");
    let mut cells: Vec<(String, String)> = sheet
        .grid()
        .into_iter()
        .map(|(col, row, cell)| (a1(col, row), cell.v.clone()))
        .collect();
    cells.sort();
    cells
}

#[test]
fn engine_loro_a1_addresses_parse_and_format() {
    assert_eq!(column_letters(1), "A");
    assert_eq!(column_letters(27), "AA");
    assert_eq!(column_letters(16_384), "XFD");
    assert_eq!(a1(28, 7), "AB7");
    assert_eq!(parse_a1(" $ab$7 "), Some((28, 7)));
    assert_eq!(parse_a1("A0"), None);
    assert_eq!(parse_a1("ABCD1"), None);
    assert_eq!(parse_a1("12"), None);
    let range = A1Range::parse("C5:A1").expect("range");
    assert_eq!((range.start, range.end), ((1, 1), (3, 5)));
    assert_eq!(A1Range::parse("B2").expect("cell").end, (2, 2));
    assert_eq!(A1Range::parse("B2:"), None);
}

#[test]
fn engine_loro_sheet_node_ids_name_a_sheet_row_or_cell() {
    assert_eq!(
        parse_node("sheet[Q1]"),
        Some(("Q1".to_owned(), SheetTarget::Sheet))
    );
    assert_eq!(
        parse_node("sheet[a\\]b]/row[3]"),
        Some(("a]b".to_owned(), SheetTarget::Row(3)))
    );
    assert_eq!(
        parse_node("sheet[S]/C4"),
        Some(("S".to_owned(), SheetTarget::Cell(3, 4)))
    );
    assert_eq!(parse_node("sheet[S]/row[0]"), None);
    assert_eq!(parse_node("sheet[S]x"), None);
    assert_eq!(parse_node("sheet[unterminated"), None);
    assert_eq!(parse_node("block[1]"), None);
}

#[test]
fn engine_loro_from_grid_derives_addresses_from_id_positions() {
    let model = workbook();
    let sheet = model.sheet("Data").expect("sheet");
    assert_eq!((sheet.rows.len(), sheet.cols.len()), (5, 2));
    assert_eq!(model.names(), vec!["Data".to_owned()]);
    assert_eq!(sheet.merge_a1(&sheet.merges[0]).as_deref(), Some("A4:B5"));
    let key = sheet::cell_key(&sheet.rows[2], &sheet.cols[1]);
    assert_eq!(sheet.position(&key), Some((2, 3)));
    assert_eq!(sheet.a1_of(&key).as_deref(), Some("B3"));
    assert_eq!(sheet.a1_of("nope:nope"), None);
    assert_eq!(sheet.ids_at(3, 1), None);
    assert_eq!(
        a1_cells(&model, "Data"),
        vec![
            ("A1".to_owned(), "2".to_owned()),
            ("B3".to_owned(), "=A1*2".to_owned())
        ]
    );
}

#[test]
fn engine_loro_a_workbook_round_trips_through_the_engine() {
    let model = ModelView::Sheet(workbook());
    let doc = LoroEngine.seed(&model).expect("seed");
    assert_eq!(doc.export_model().expect("export"), model);
    let reopened = LoroEngine
        .open(&doc.snapshot().expect("snapshot"))
        .expect("open");
    assert_eq!(reopened.export_model().expect("export"), model);
    let mut edited = workbook();
    edited.sheets[0] = Sheet::from_grid("Data", [(1, 1, input("3"))], &[]);
    let edited = ModelView::Sheet(edited);
    let update = doc
        .update_to_model(&edited)
        .expect("diff")
        .expect("changed");
    let mut replica = reopened;
    replica.apply_update(&update).expect("apply");
    assert_eq!(replica.export_model().expect("export"), edited);
}

#[test]
fn engine_loro_seed_sheets_fills_only_an_empty_or_placeholder_workbook() {
    let doc = LoroDoc::new();
    let placeholder = SheetModel {
        sheets: vec![Sheet::from_grid("Sheet1", [], &[])],
    };
    assert!(seed_sheets(&doc, &placeholder).expect("empty"));
    assert!(seed_sheets(&doc, &workbook()).expect("placeholder"));
    assert!(!seed_sheets(&doc, &placeholder).expect("held"));
    assert_eq!(
        sheet::read(&doc).expect("read").names(),
        vec!["Data".to_owned()]
    );
}

#[test]
fn engine_loro_cell_edits_grow_the_sheet_and_insert_rows_by_id() {
    let doc = seed_model(&ModelView::Sheet(workbook())).expect("seed");
    let map = sheet_map(&doc, "Data").expect("sheet");
    assert!(sheet_map(&doc, "Other").is_none());
    let key = ensure_cell_key(&map, 4, 7).expect("grow");
    set_cell_input(&map, &key, "far").expect("set");
    let a1_key = ensure_cell_key(&map, 1, 1).expect("existing");
    set_cell_input(&map, &a1_key, "").expect("clear");
    let row = insert_row_after(&map, 0).expect("row");
    assert_eq!(row_number(&map, &row), Some(1));
    assert_eq!(row_number(&map, "missing"), None);
    assert!(ensure_cell_key(&map, 0, 1).is_err());
    doc.commit();
    let model = sheet::read(&doc).expect("read");
    assert_eq!(
        a1_cells(&model, "Data"),
        vec![
            ("B4".to_owned(), "=A1*2".to_owned()),
            ("D8".to_owned(), "far".to_owned())
        ]
    );
    assert_eq!(
        model.sheets[0]
            .merge_a1(&model.sheets[0].merges[0])
            .as_deref(),
        Some("A5:B6")
    );
}

#[test]
fn engine_loro_concurrent_row_inserts_and_cell_edits_converge() {
    let left = seed_model(&ModelView::Sheet(workbook())).expect("seed");
    let right = LoroDoc::new();
    rich::configure(&right);
    right
        .import(&left.export(ExportMode::Snapshot).expect("snapshot"))
        .expect("import");
    right.set_peer_id(55).expect("peer");
    let base = left.oplog_vv();
    let map = sheet_map(&left, "Data").expect("left sheet");
    insert_row_after(&map, 0).expect("left row");
    left.commit();
    let map = sheet_map(&right, "Data").expect("right sheet");
    let key = ensure_cell_key(&map, 1, 1).expect("key");
    set_cell_input(&map, &key, "9").expect("right cell");
    right.commit();
    left.import(&right.export(ExportMode::updates(&base)).expect("r"))
        .expect("into left");
    right
        .import(&left.export(ExportMode::updates(&base)).expect("l"))
        .expect("into right");
    let merged = sheet::read(&left).expect("read");
    assert_eq!(merged, sheet::read(&right).expect("read"));
    assert_eq!(
        a1_cells(&merged, "Data"),
        vec![
            ("A2".to_owned(), "9".to_owned()),
            ("B4".to_owned(), "=A1*2".to_owned())
        ]
    );
}

#[test]
fn engine_loro_write_sheets_replaces_every_sheet() {
    let doc = seed_model(&ModelView::Sheet(workbook())).expect("seed");
    let next = SheetModel {
        sheets: vec![Sheet::from_grid("Other", [(1, 1, input("x"))], &[])],
    };
    write_sheets(&doc, &next).expect("write");
    doc.commit();
    assert_eq!(sheet::read(&doc).expect("read"), next);
}

fn change(sheet: &str, col: u32, row: u32, v: &str) -> SheetChange {
    SheetChange {
        sheet: sheet.to_owned(),
        col,
        row,
        v: v.to_owned(),
    }
}

#[test]
fn engine_loro_sheet_validation_refuses_each_bad_change() {
    let names = vec!["Data".to_owned()];
    assert_eq!(
        validate_sheet_changes(&[change("Data", 1, 1, "=SUM(A2:A3)")], &names),
        Ok(())
    );
    assert_eq!(
        validate_sheet_changes(&[change("Nope", 1, 1, "x")], &names),
        Err(SheetValidationError::UnknownSheet {
            sheet: "Nope".to_owned()
        })
    );
    assert_eq!(
        validate_sheet_changes(&[change("Data", 0, 1, "x")], &names),
        Err(SheetValidationError::OutOfBounds { col: 0, row: 1 })
    );
    assert_eq!(
        validate_sheet_changes(&[change("Data", 1, 1_048_577, "x")], &names),
        Err(SheetValidationError::OutOfBounds {
            col: 1,
            row: 1_048_577
        })
    );
    assert_eq!(
        validate_sheet_changes(&[change("Data", 2, 3, &"x".repeat(32_768))], &names),
        Err(SheetValidationError::TooLong {
            cell: "Data!B3".to_owned(),
            chars: 32_768
        })
    );
    assert!(matches!(
        validate_sheet_changes(&[change("Data", 1, 1, "=SUM(")], &names),
        Err(SheetValidationError::Formula { cell, .. }) if cell == "Data!A1"
    ));
    let many = vec![change("Data", 1, 1, "x"); 100_001];
    assert_eq!(
        validate_sheet_changes(&many, &names),
        Err(SheetValidationError::TooManyChanges {
            count: 100_001,
            max: 100_000
        })
    );
}
