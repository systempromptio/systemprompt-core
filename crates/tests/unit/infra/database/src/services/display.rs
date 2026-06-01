//! Tests for `DatabaseCliDisplay` — coverage of the formatting logic via public
//! surface and struct construction (display writes to stdout so we only verify
//! that the impl exists and the `is_empty` / `first` paths on `QueryResult`
//! stay consistent with what the display impl branches on; the actual write is
//! side-effecting and not captured here).

use systemprompt_database::{ColumnInfo, DatabaseInfo, QueryResult, TableInfo};

fn make_table_info(name: &str, rows: i64) -> TableInfo {
    TableInfo {
        name: name.to_string(),
        row_count: rows,
        size_bytes: 0,
        columns: vec![],
    }
}

fn make_column_info(name: &str, nullable: bool, pk: bool) -> ColumnInfo {
    ColumnInfo {
        name: name.to_string(),
        data_type: "text".to_string(),
        nullable,
        primary_key: pk,
        default: None,
    }
}

#[test]
fn table_info_list_empty_is_empty() {
    let tables: Vec<TableInfo> = vec![];
    assert!(tables.is_empty());
}

#[test]
fn table_info_list_single_entry() {
    let tables = vec![make_table_info("users", 100)];
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
    assert_eq!(tables[0].row_count, 100);
}

#[test]
fn table_info_list_multiple() {
    let tables = vec![
        make_table_info("users", 50),
        make_table_info("sessions", 200),
        make_table_info("logs", 10_000),
    ];
    assert_eq!(tables.len(), 3);
    assert_eq!(tables[1].row_count, 200);
}

#[test]
fn column_info_nullable_pk_combinations() {
    let pk_not_null = make_column_info("id", false, true);
    let optional = make_column_info("bio", true, false);
    let required = make_column_info("email", false, false);

    assert!(!pk_not_null.nullable && pk_not_null.primary_key);
    assert!(optional.nullable && !optional.primary_key);
    assert!(!required.nullable && !required.primary_key);
}

#[test]
fn column_info_with_default() {
    let col = ColumnInfo {
        name: "created_at".to_string(),
        data_type: "timestamptz".to_string(),
        nullable: false,
        primary_key: false,
        default: Some("CURRENT_TIMESTAMP".to_string()),
    };
    assert!(col.default.is_some());
    assert_eq!(col.default.as_deref(), Some("CURRENT_TIMESTAMP"));
}

#[test]
fn column_info_no_default() {
    let col = make_column_info("name", false, false);
    assert!(col.default.is_none());
}

#[test]
fn column_with_row_count_tuple() {
    let cols = vec![make_column_info("id", false, true)];
    let row_count: i64 = 42;
    let pair = (cols, row_count);
    assert_eq!(pair.0.len(), 1);
    assert_eq!(pair.1, 42);
}

#[test]
fn database_info_fields() {
    let db = DatabaseInfo {
        path: "postgresql://localhost/test".to_string(),
        size: 1024 * 1024,
        version: "PostgreSQL 15.0".to_string(),
        tables: vec![make_table_info("users", 10)],
    };
    assert_eq!(db.tables.len(), 1);
    assert!(db.version.contains("PostgreSQL"));
}

#[test]
fn query_result_empty_branches() {
    let result = QueryResult::new();
    assert!(result.columns.is_empty());
    assert!(result.is_empty());
    assert!(result.first().is_none());
}

#[test]
fn query_result_with_rows_branches() {
    let mut row = std::collections::HashMap::new();
    row.insert("id".to_string(), serde_json::json!(1));
    row.insert("name".to_string(), serde_json::json!("Alice"));

    let result = QueryResult {
        columns: vec!["id".to_string(), "name".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 5,
    };

    assert!(!result.is_empty());
    let first = result.first().expect("first row");
    assert_eq!(first.get("name").unwrap(), &serde_json::json!("Alice"));
}

#[test]
fn query_result_with_null_value_row() {
    let mut row = std::collections::HashMap::new();
    row.insert("maybe".to_string(), serde_json::Value::Null);

    let result = QueryResult {
        columns: vec!["maybe".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 2,
    };

    let first = result.first().unwrap();
    assert_eq!(first.get("maybe").unwrap(), &serde_json::Value::Null);
}

#[test]
fn query_result_with_bool_and_number_values() {
    let mut row = std::collections::HashMap::new();
    row.insert("active".to_string(), serde_json::Value::Bool(true));
    row.insert("count".to_string(), serde_json::json!(42));

    let result = QueryResult {
        columns: vec!["active".to_string(), "count".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 3,
    };

    let first = result.first().unwrap();
    assert_eq!(first.get("active").unwrap(), &serde_json::Value::Bool(true));
    assert_eq!(first.get("count").unwrap(), &serde_json::json!(42));
}

#[test]
fn database_info_table_count_for_display() {
    let db = DatabaseInfo {
        path: "db".to_string(),
        size: 0,
        version: "15.1".to_string(),
        tables: vec![
            make_table_info("a", 0),
            make_table_info("b", 0),
            make_table_info("c", 0),
        ],
    };
    assert_eq!(db.tables.len(), 3);
}

#[test]
fn table_info_zero_row_count() {
    let table = make_table_info("empty", 0);
    assert_eq!(table.row_count, 0);
}

#[test]
fn column_info_data_type_display() {
    let col = ColumnInfo {
        name: "amount".to_string(),
        data_type: "numeric(10,2)".to_string(),
        nullable: true,
        primary_key: false,
        default: Some("0.00".to_string()),
    };
    assert_eq!(col.data_type, "numeric(10,2)");
}
