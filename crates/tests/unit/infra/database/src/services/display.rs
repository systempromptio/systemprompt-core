//! Unit tests for DatabaseCliDisplay trait implementations
//!
//! Note: These tests verify that display methods can be called without panicking.
//! The actual output goes to stdout and is not captured in unit tests.

use std::collections::HashMap;
use systemprompt_core_database::{
    ColumnInfo, DatabaseCliDisplay, DatabaseInfo, QueryResult, TableInfo,
};

// ============================================================================
// Vec<TableInfo> Display Tests
// ============================================================================

#[test]
fn test_empty_tables_display_no_panic() {
    let tables: Vec<TableInfo> = vec![];
    // Should not panic
    tables.display_with_cli();
}

#[test]
fn test_single_table_display_no_panic() {
    let tables = vec![TableInfo {
        name: "users".to_string(),
        row_count: 100,
        size_bytes: 0,
        columns: vec![],
    }];
    // Should not panic
    tables.display_with_cli();
}

#[test]
fn test_multiple_tables_display_no_panic() {
    let tables = vec![
        TableInfo {
            name: "users".to_string(),
            row_count: 100,
            size_bytes: 0,
            columns: vec![],
        },
        TableInfo {
            name: "posts".to_string(),
            row_count: 500,
            size_bytes: 0,
            columns: vec![],
        },
        TableInfo {
            name: "comments".to_string(),
            row_count: 1000,
            size_bytes: 0,
            columns: vec![],
        },
    ];
    // Should not panic
    tables.display_with_cli();
}

#[test]
fn test_table_with_zero_rows_display_no_panic() {
    let tables = vec![TableInfo {
        name: "empty_table".to_string(),
        row_count: 0,
        size_bytes: 0,
        columns: vec![],
    }];
    // Should not panic
    tables.display_with_cli();
}

// ============================================================================
// (Vec<ColumnInfo>, i64) Display Tests
// ============================================================================

#[test]
fn test_empty_columns_display_no_panic() {
    let columns: Vec<ColumnInfo> = vec![];
    let tuple = (columns, 0i64);
    // Should not panic
    tuple.display_with_cli();
}

#[test]
fn test_single_column_display_no_panic() {
    let columns = vec![ColumnInfo {
        name: "id".to_string(),
        data_type: "uuid".to_string(),
        nullable: false,
        primary_key: true,
        default: None,
    }];
    let tuple = (columns, 100i64);
    // Should not panic
    tuple.display_with_cli();
}

#[test]
fn test_nullable_column_display_no_panic() {
    let columns = vec![ColumnInfo {
        name: "description".to_string(),
        data_type: "text".to_string(),
        nullable: true,
        primary_key: false,
        default: None,
    }];
    let tuple = (columns, 50i64);
    // Should not panic
    tuple.display_with_cli();
}

#[test]
fn test_column_with_default_display_no_panic() {
    let columns = vec![ColumnInfo {
        name: "created_at".to_string(),
        data_type: "timestamp".to_string(),
        nullable: false,
        primary_key: false,
        default: Some("CURRENT_TIMESTAMP".to_string()),
    }];
    let tuple = (columns, 25i64);
    // Should not panic
    tuple.display_with_cli();
}

#[test]
fn test_multiple_columns_display_no_panic() {
    let columns = vec![
        ColumnInfo {
            name: "id".to_string(),
            data_type: "uuid".to_string(),
            nullable: false,
            primary_key: true,
            default: None,
        },
        ColumnInfo {
            name: "email".to_string(),
            data_type: "varchar".to_string(),
            nullable: false,
            primary_key: false,
            default: None,
        },
        ColumnInfo {
            name: "age".to_string(),
            data_type: "integer".to_string(),
            nullable: true,
            primary_key: false,
            default: Some("0".to_string()),
        },
    ];
    let tuple = (columns, 1000i64);
    // Should not panic
    tuple.display_with_cli();
}

// ============================================================================
// DatabaseInfo Display Tests
// ============================================================================

#[test]
fn test_database_info_display_no_panic() {
    let db_info = DatabaseInfo {
        path: "postgresql://localhost/test".to_string(),
        size: 1024,
        version: "PostgreSQL 15.0".to_string(),
        tables: vec![],
    };
    // Should not panic
    db_info.display_with_cli();
}

#[test]
fn test_database_info_with_tables_display_no_panic() {
    let db_info = DatabaseInfo {
        path: "db_path".to_string(),
        size: 2048,
        version: "14.0".to_string(),
        tables: vec![
            TableInfo {
                name: "table1".to_string(),
                row_count: 10,
                size_bytes: 0,
                columns: vec![],
            },
            TableInfo {
                name: "table2".to_string(),
                row_count: 20,
                size_bytes: 0,
                columns: vec![],
            },
        ],
    };
    // Should not panic
    db_info.display_with_cli();
}

// ============================================================================
// QueryResult Display Tests
// ============================================================================

#[test]
fn test_empty_query_result_display_no_panic() {
    let result = QueryResult::new();
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_columns_no_rows_display_no_panic() {
    let result = QueryResult {
        columns: vec!["id".to_string(), "name".to_string()],
        rows: vec![],
        row_count: 0,
        execution_time_ms: 10,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_data_display_no_panic() {
    let mut row = HashMap::new();
    row.insert("id".to_string(), serde_json::json!(1));
    row.insert("name".to_string(), serde_json::json!("Alice"));

    let result = QueryResult {
        columns: vec!["id".to_string(), "name".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 25,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_null_values_display_no_panic() {
    let mut row = HashMap::new();
    row.insert("id".to_string(), serde_json::json!(1));
    row.insert("nullable".to_string(), serde_json::Value::Null);

    let result = QueryResult {
        columns: vec!["id".to_string(), "nullable".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 5,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_boolean_display_no_panic() {
    let mut row = HashMap::new();
    row.insert("active".to_string(), serde_json::json!(true));
    row.insert("deleted".to_string(), serde_json::json!(false));

    let result = QueryResult {
        columns: vec!["active".to_string(), "deleted".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 3,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_number_display_no_panic() {
    let mut row = HashMap::new();
    row.insert("count".to_string(), serde_json::json!(42));
    row.insert("price".to_string(), serde_json::json!(19.99));

    let result = QueryResult {
        columns: vec!["count".to_string(), "price".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 7,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_array_display_no_panic() {
    let mut row = HashMap::new();
    row.insert("tags".to_string(), serde_json::json!(["rust", "database"]));

    let result = QueryResult {
        columns: vec!["tags".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 4,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_object_display_no_panic() {
    let mut row = HashMap::new();
    row.insert("metadata".to_string(), serde_json::json!({"key": "value"}));

    let result = QueryResult {
        columns: vec!["metadata".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 6,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_with_multiple_rows_display_no_panic() {
    let mut row1 = HashMap::new();
    row1.insert("id".to_string(), serde_json::json!(1));

    let mut row2 = HashMap::new();
    row2.insert("id".to_string(), serde_json::json!(2));

    let mut row3 = HashMap::new();
    row3.insert("id".to_string(), serde_json::json!(3));

    let result = QueryResult {
        columns: vec!["id".to_string()],
        rows: vec![row1, row2, row3],
        row_count: 3,
        execution_time_ms: 15,
    };
    // Should not panic
    result.display_with_cli();
}

#[test]
fn test_query_result_missing_column_value_display_no_panic() {
    let mut row = HashMap::new();
    row.insert("id".to_string(), serde_json::json!(1));
    // Note: "name" column is missing from the row

    let result = QueryResult {
        columns: vec!["id".to_string(), "name".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 2,
    };
    // Should not panic - missing column should display as NULL
    result.display_with_cli();
}
