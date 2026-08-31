//! `TableArtifact` — the wire shape a table renders as.
//!
//! `to_response` is what a client actually receives. Two things in it are not
//! stored but derived, and both are read directly by the renderer: the
//! `x-artifact-type` discriminator it dispatches on, and `count`, which it
//! shows without recounting the rows.

use serde_json::json;
use systemprompt_models::artifacts::types::ColumnType;
use systemprompt_models::artifacts::{Column, TableArtifact};

fn columns() -> Vec<Column> {
    vec![
        Column::new("id", ColumnType::String),
        Column::new("total", ColumnType::Currency),
    ]
}

fn rows() -> Vec<serde_json::Value> {
    vec![
        json!({"id": "a", "total": 10}),
        json!({"id": "b", "total": 20}),
        json!({"id": "c", "total": 30}),
    ]
}

// Why: `count` is derived from the rows rather than stored, and the renderer
// displays it without recounting. Stored separately it could drift, and a
// table would report a different size than it shows.
#[test]
fn the_reported_count_is_the_number_of_rows_carried() {
    let response = TableArtifact::new(columns())
        .with_rows(rows())
        .to_response();

    assert_eq!(response["count"], 3);
    assert_eq!(
        response["items"].as_array().map(Vec::len),
        Some(3),
        "count and items must describe the same table"
    );
}

#[test]
fn an_empty_table_reports_zero_rather_than_omitting_the_count() {
    let response = TableArtifact::new(columns()).to_response();

    assert_eq!(
        response["count"], 0,
        "a table with no rows is still a table with a count"
    );
    assert!(response["items"].as_array().is_some_and(Vec::is_empty));
}

// Why: `x-artifact-type` is the discriminator a client dispatches on. Renamed
// or absent, the renderer cannot tell a table from any other artifact and
// falls back to raw JSON.
#[test]
fn the_wire_carries_the_artifact_type_discriminator() {
    let response = TableArtifact::new(columns()).to_response();

    assert_eq!(
        response["x-artifact-type"], "table",
        "the discriminator must be present under its hyphenated wire name"
    );
    assert!(
        response.get("artifact_type").is_none(),
        "the Rust field name must not leak onto the wire alongside it"
    );
}

// Why: columns define what the renderer draws and in what order. A reordering
// or a dropped column silently changes the table a reader sees.
#[test]
fn the_columns_reach_the_wire_in_the_order_they_were_given() {
    let response = TableArtifact::new(columns())
        .with_rows(rows())
        .to_response();

    let names: Vec<&str> = response["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();

    assert_eq!(names, vec!["id", "total"]);
}

// Why: the column type tells the renderer how to format a cell — currency is
// not an integer. It travels under `column_type`, not the Rust field name.
#[test]
fn each_column_carries_its_type_under_the_wire_name() {
    let response = TableArtifact::new(columns()).to_response();
    let first = &response["columns"].as_array().expect("columns")[0];

    assert!(
        first.get("column_type").is_some(),
        "the renderer reads column_type: {first}"
    );
    assert!(
        first.get("kind").is_none(),
        "the Rust field name must not appear on the wire"
    );
}

#[test]
fn a_title_is_carried_when_set_and_omitted_when_not() {
    let untitled = TableArtifact::new(columns()).to_response();
    assert!(
        untitled.get("title").is_none(),
        "an absent title is omitted rather than sent as null"
    );

    let titled = TableArtifact::new(columns())
        .with_title("Invoices")
        .to_response();
    assert_eq!(titled["title"], "Invoices");
}

// Why: the rows are passed through untouched. A renderer reads the values it
// was given, so any reshaping here would show a reader something the caller
// did not send.
#[test]
fn row_values_reach_the_wire_unchanged() {
    let response = TableArtifact::new(columns())
        .with_rows(rows())
        .to_response();
    let items = response["items"].as_array().expect("items");

    assert_eq!(items[0]["id"], "a");
    assert_eq!(items[2]["total"], 30);
}
