use systemprompt_models::documents::{CellRange, CellRangeError, MAX_CELL_COLUMN, MAX_CELL_ROW};

fn parse(text: &str) -> Result<CellRange, CellRangeError> {
    text.parse()
}

#[test]
fn a_single_cell_and_a_range_parse_and_display() {
    let one = parse("B2").expect("B2");
    assert_eq!((one.start(), one.end()), ((2, 2), (2, 2)));
    assert_eq!(one.to_string(), "B2");
    let range = parse("B2:D9").expect("B2:D9");
    assert_eq!((range.start(), range.end()), ((2, 2), (4, 9)));
    assert_eq!(range.to_string(), "B2:D9");
    assert!(range.contains(3, 5));
    assert!(!range.contains(5, 5));
}

#[test]
fn letters_are_read_in_any_case_and_written_upper() {
    assert_eq!(parse("b2:aa10").expect("lower").to_string(), "B2:AA10");
    assert_eq!(parse("C3:C3").expect("degenerate").to_string(), "C3");
}

#[test]
fn the_sheet_limits_are_inclusive() {
    let last = parse("A1:XFD1048576").expect("whole sheet");
    assert_eq!(last.end(), (MAX_CELL_COLUMN, MAX_CELL_ROW));
    assert_eq!(last.to_string(), "A1:XFD1048576");
    assert_eq!(parse("XFE1"), Err(CellRangeError::ColumnOutOfRange));
    assert_eq!(parse("AAAA1"), Err(CellRangeError::ColumnOutOfRange));
    assert_eq!(parse("A1048577"), Err(CellRangeError::RowOutOfRange));
    assert_eq!(parse("A99999999"), Err(CellRangeError::RowOutOfRange));
    assert_eq!(
        CellRange::new((1, 1), (MAX_CELL_COLUMN + 1, 1)),
        Err(CellRangeError::ColumnOutOfRange)
    );
}

#[test]
fn a_reversed_range_is_refused() {
    for reversed in ["D9:B2", "D2:B9", "B9:D2"] {
        assert_eq!(parse(reversed), Err(CellRangeError::Reversed), "{reversed}");
    }
    assert_eq!(
        CellRange::new((4, 9), (2, 2)),
        Err(CellRangeError::Reversed)
    );
}

#[test]
fn garbage_is_malformed() {
    for bad in [
        "",
        "B",
        "2",
        "B0",
        "B02",
        "B2:",
        ":B2",
        " B2",
        "B2 ",
        "$B$2",
        "B2:C3:D4",
        "Ä1",
        "B-2",
        "Sheet1!B2",
        "B2..D9",
    ] {
        assert_eq!(parse(bad), Err(CellRangeError::Malformed), "{bad:?}");
    }
}

#[test]
fn serde_uses_the_a1_text_and_validates_it() {
    let range = parse("B2:D9").expect("range");
    assert_eq!(
        serde_json::to_value(range).expect("ser"),
        serde_json::json!("B2:D9")
    );
    let back: CellRange = serde_json::from_value(serde_json::json!("b2:d9")).expect("de");
    assert_eq!(back, range);
    assert!(serde_json::from_value::<CellRange>(serde_json::json!("D9:B2")).is_err());
    assert!(serde_json::from_value::<CellRange>(serde_json::json!(42)).is_err());
}
