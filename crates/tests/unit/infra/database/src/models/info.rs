//! Unit tests for DatabaseInfo, TableInfo, ColumnInfo, and IndexInfo models

use systemprompt_database::{ColumnInfo, DatabaseInfo, IndexInfo, TableInfo};

// ============================================================================
// ColumnInfo Tests
// ============================================================================

#[test]
fn test_column_info_creation() {
    let column = ColumnInfo {
        name: "id".to_string(),
        data_type: "uuid".to_string(),
        nullable: false,
        primary_key: true,
        default: None,
    };

    assert_eq!(column.name, "id");
    assert_eq!(column.data_type, "uuid");
    assert!(!column.nullable);
    assert!(column.primary_key);
    assert!(column.default.is_none());
}

#[test]
fn test_column_info_with_default() {
    let column = ColumnInfo {
        name: "created_at".to_string(),
        data_type: "timestamp".to_string(),
        nullable: false,
        primary_key: false,
        default: Some("CURRENT_TIMESTAMP".to_string()),
    };

    assert_eq!(column.default, Some("CURRENT_TIMESTAMP".to_string()));
}

#[test]
fn test_column_info_nullable() {
    let column = ColumnInfo {
        name: "description".to_string(),
        data_type: "text".to_string(),
        nullable: true,
        primary_key: false,
        default: None,
    };

    assert!(column.nullable);
}

#[test]
fn test_column_info_debug() {
    let column = ColumnInfo {
        name: "status".to_string(),
        data_type: "varchar".to_string(),
        nullable: false,
        primary_key: false,
        default: None,
    };

    let debug = format!("{:?}", column);
    assert!(debug.contains("ColumnInfo"));
    assert!(debug.contains("status"));
}

#[test]
fn test_column_info_clone() {
    let column = ColumnInfo {
        name: "email".to_string(),
        data_type: "varchar".to_string(),
        nullable: false,
        primary_key: false,
        default: None,
    };

    let cloned = column.clone();
    assert_eq!(column.name, cloned.name);
    assert_eq!(column.data_type, cloned.data_type);
}

#[test]
fn test_column_info_serialization() {
    let column = ColumnInfo {
        name: "amount".to_string(),
        data_type: "decimal".to_string(),
        nullable: true,
        primary_key: false,
        default: Some("0.00".to_string()),
    };

    let json = serde_json::to_string(&column).expect("Should serialize");
    assert!(json.contains("\"name\":\"amount\""));
    assert!(json.contains("\"data_type\":\"decimal\""));
}

#[test]
fn test_column_info_deserialization() {
    let json = r#"{"name":"id","data_type":"integer","nullable":false,"primary_key":true,"default":null}"#;
    let column: ColumnInfo = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(column.name, "id");
    assert_eq!(column.data_type, "integer");
    assert!(!column.nullable);
    assert!(column.primary_key);
    assert!(column.default.is_none());
}

// ============================================================================
// TableInfo Tests
// ============================================================================

#[test]
fn test_table_info_creation() {
    let table = TableInfo {
        name: "users".to_string(),
        row_count: 100,
        size_bytes: 0,
        columns: vec![],
    };

    assert_eq!(table.name, "users");
    assert_eq!(table.row_count, 100);
    assert!(table.columns.is_empty());
}

#[test]
fn test_table_info_with_columns() {
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
    ];

    let table = TableInfo {
        name: "users".to_string(),
        row_count: 50,
        size_bytes: 0,
        columns,
    };

    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.columns[0].name, "id");
    assert_eq!(table.columns[1].name, "email");
}

#[test]
fn test_table_info_zero_rows() {
    let table = TableInfo {
        name: "empty_table".to_string(),
        row_count: 0,
        size_bytes: 0,
        columns: vec![],
    };

    assert_eq!(table.row_count, 0);
}

#[test]
fn test_table_info_negative_row_count() {
    let table = TableInfo {
        name: "test".to_string(),
        row_count: -1,
        size_bytes: 0,
        columns: vec![],
    };

    assert_eq!(table.row_count, -1);
}

#[test]
fn test_table_info_debug() {
    let table = TableInfo {
        name: "products".to_string(),
        row_count: 1000,
        size_bytes: 0,
        columns: vec![],
    };

    let debug = format!("{:?}", table);
    assert!(debug.contains("TableInfo"));
    assert!(debug.contains("products"));
}

#[test]
fn test_table_info_serialization() {
    let table = TableInfo {
        name: "orders".to_string(),
        row_count: 500,
        size_bytes: 0,
        columns: vec![],
    };

    let json = serde_json::to_string(&table).expect("Should serialize");
    assert!(json.contains("\"name\":\"orders\""));
    assert!(json.contains("\"row_count\":500"));
}

// ============================================================================
// DatabaseInfo Tests
// ============================================================================

#[test]
fn test_database_info_creation() {
    let db_info = DatabaseInfo {
        path: "postgresql://localhost/test".to_string(),
        size: 1024,
        version: "PostgreSQL 15.0".to_string(),
        tables: vec![],
    };

    assert_eq!(db_info.path, "postgresql://localhost/test");
    assert_eq!(db_info.size, 1024);
    assert_eq!(db_info.version, "PostgreSQL 15.0");
    assert!(db_info.tables.is_empty());
}

#[test]
fn test_database_info_with_tables() {
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
    ];

    let db_info = DatabaseInfo {
        path: "db".to_string(),
        size: 2048,
        version: "15.0".to_string(),
        tables,
    };

    assert_eq!(db_info.tables.len(), 2);
    assert_eq!(db_info.tables[0].name, "users");
    assert_eq!(db_info.tables[1].name, "posts");
}

#[test]
fn test_database_info_zero_size() {
    let db_info = DatabaseInfo {
        path: "empty".to_string(),
        size: 0,
        version: "1.0".to_string(),
        tables: vec![],
    };

    assert_eq!(db_info.size, 0);
}

#[test]
fn test_database_info_debug() {
    let db_info = DatabaseInfo {
        path: "test_db".to_string(),
        size: 4096,
        version: "14.0".to_string(),
        tables: vec![],
    };

    let debug = format!("{:?}", db_info);
    assert!(debug.contains("DatabaseInfo"));
    assert!(debug.contains("test_db"));
}

#[test]
fn test_database_info_clone() {
    let db_info = DatabaseInfo {
        path: "original".to_string(),
        size: 1000,
        version: "1.0".to_string(),
        tables: vec![TableInfo {
            name: "t1".to_string(),
            row_count: 10,
            size_bytes: 0,
            columns: vec![],
        }],
    };

    let cloned = db_info.clone();
    assert_eq!(db_info.path, cloned.path);
    assert_eq!(db_info.tables.len(), cloned.tables.len());
}

#[test]
fn test_database_info_serialization() {
    let db_info = DatabaseInfo {
        path: "prod_db".to_string(),
        size: 8192,
        version: "15.2".to_string(),
        tables: vec![],
    };

    let json = serde_json::to_string(&db_info).expect("Should serialize");
    assert!(json.contains("\"path\":\"prod_db\""));
    assert!(json.contains("\"size\":8192"));
    assert!(json.contains("\"version\":\"15.2\""));
}

#[test]
fn test_database_info_deserialization() {
    let json = r#"{"path":"test","size":512,"version":"14.0","tables":[]}"#;
    let db_info: DatabaseInfo = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(db_info.path, "test");
    assert_eq!(db_info.size, 512);
    assert_eq!(db_info.version, "14.0");
    assert!(db_info.tables.is_empty());
}

// ============================================================================
// IndexInfo Tests
// ============================================================================

#[test]
fn test_index_info_creation() {
    let index = IndexInfo {
        name: "idx_users_email".to_string(),
        columns: vec!["email".to_string()],
        unique: true,
    };

    assert_eq!(index.name, "idx_users_email");
    assert_eq!(index.columns.len(), 1);
    assert_eq!(index.columns[0], "email");
    assert!(index.unique);
}

#[test]
fn test_index_info_non_unique() {
    let index = IndexInfo {
        name: "idx_posts_created_at".to_string(),
        columns: vec!["created_at".to_string()],
        unique: false,
    };

    assert!(!index.unique);
}

#[test]
fn test_index_info_multiple_columns() {
    let index = IndexInfo {
        name: "idx_composite".to_string(),
        columns: vec![
            "user_id".to_string(),
            "post_id".to_string(),
            "created_at".to_string(),
        ],
        unique: true,
    };

    assert_eq!(index.columns.len(), 3);
    assert_eq!(index.columns[0], "user_id");
    assert_eq!(index.columns[1], "post_id");
    assert_eq!(index.columns[2], "created_at");
}

#[test]
fn test_index_info_empty_columns() {
    let index = IndexInfo {
        name: "idx_empty".to_string(),
        columns: vec![],
        unique: false,
    };

    assert!(index.columns.is_empty());
}

#[test]
fn test_index_info_debug() {
    let index = IndexInfo {
        name: "idx_test".to_string(),
        columns: vec!["col1".to_string()],
        unique: true,
    };

    let debug = format!("{:?}", index);
    assert!(debug.contains("IndexInfo"));
    assert!(debug.contains("idx_test"));
}

#[test]
fn test_index_info_clone() {
    let index = IndexInfo {
        name: "idx_original".to_string(),
        columns: vec!["a".to_string(), "b".to_string()],
        unique: true,
    };

    let cloned = index.clone();
    assert_eq!(index.name, cloned.name);
    assert_eq!(index.columns, cloned.columns);
    assert_eq!(index.unique, cloned.unique);
}

#[test]
fn test_index_info_serialization() {
    let index = IndexInfo {
        name: "idx_serialize".to_string(),
        columns: vec!["col".to_string()],
        unique: false,
    };

    let json = serde_json::to_string(&index).expect("Should serialize");
    assert!(json.contains("\"name\":\"idx_serialize\""));
    assert!(json.contains("\"unique\":false"));
}

#[test]
fn test_index_info_deserialization() {
    let json = r#"{"name":"idx_deser","columns":["a","b"],"unique":true}"#;
    let index: IndexInfo = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(index.name, "idx_deser");
    assert_eq!(index.columns, vec!["a", "b"]);
    assert!(index.unique);
}
