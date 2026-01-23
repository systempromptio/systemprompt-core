//! Unit tests for FilesExtension
//!
//! Tests cover:
//! - Extension metadata (id, name, version)
//! - Schema definitions
//! - Migration weight
//! - Dependencies

use systemprompt_extension::Extension;
use systemprompt_files::FilesExtension;

#[test]
fn test_files_extension_default() {
    let ext = FilesExtension::default();
    let debug_str = format!("{:?}", ext);
    assert!(debug_str.contains("FilesExtension"));
}

#[test]
fn test_files_extension_clone() {
    let ext = FilesExtension;
    let cloned = ext;
    let _ = format!("{:?}", cloned);
}

#[test]
fn test_files_extension_copy() {
    let ext = FilesExtension;
    let copied: FilesExtension = ext;
    let _ = format!("{:?}", copied);
}

#[test]
fn test_files_extension_debug() {
    let ext = FilesExtension;
    let debug_str = format!("{:?}", ext);
    assert!(debug_str.contains("FilesExtension"));
}

#[test]
fn test_files_extension_metadata_id() {
    let ext = FilesExtension;
    let metadata = ext.metadata();
    assert_eq!(metadata.id, "files");
}

#[test]
fn test_files_extension_metadata_name() {
    let ext = FilesExtension;
    let metadata = ext.metadata();
    assert_eq!(metadata.name, "Files");
}

#[test]
fn test_files_extension_metadata_version() {
    let ext = FilesExtension;
    let metadata = ext.metadata();
    assert!(!metadata.version.is_empty());
}

#[test]
fn test_files_extension_migration_weight() {
    let ext = FilesExtension;
    let weight = ext.migration_weight();
    assert_eq!(weight, 15);
}

#[test]
fn test_files_extension_schemas_count() {
    let ext = FilesExtension;
    let schemas = ext.schemas();
    assert_eq!(schemas.len(), 3);
}

#[test]
fn test_files_extension_schemas_names() {
    let ext = FilesExtension;
    let schemas = ext.schemas();
    let names: Vec<&str> = schemas.iter().map(|s| s.table.as_str()).collect();
    assert!(names.contains(&"files"));
    assert!(names.contains(&"content_files"));
    assert!(names.contains(&"ai_image_analytics"));
}

#[test]
fn test_files_extension_files_schema_has_required_columns() {
    let ext = FilesExtension;
    let schemas = ext.schemas();
    let files_schema = schemas.iter().find(|s| s.table == "files").unwrap();
    let required = &files_schema.required_columns;
    assert!(required.contains(&"id".to_string()));
    assert!(required.contains(&"filename".to_string()));
    assert!(required.contains(&"mime_type".to_string()));
    assert!(required.contains(&"created_at".to_string()));
}

#[test]
fn test_files_extension_content_files_schema_has_required_columns() {
    let ext = FilesExtension;
    let schemas = ext.schemas();
    let schema = schemas.iter().find(|s| s.table == "content_files").unwrap();
    let required = &schema.required_columns;
    assert!(required.contains(&"id".to_string()));
    assert!(required.contains(&"content_id".to_string()));
    assert!(required.contains(&"file_id".to_string()));
}

#[test]
fn test_files_extension_dependencies() {
    let ext = FilesExtension;
    let deps = ext.dependencies();
    assert_eq!(deps.len(), 1);
    assert!(deps.contains(&"users"));
}

#[test]
fn test_files_extension_schemas_have_sql() {
    use systemprompt_extension::SchemaSource;

    let ext = FilesExtension;
    let schemas = ext.schemas();
    for schema in schemas {
        match &schema.sql {
            SchemaSource::Inline(sql) => {
                assert!(!sql.is_empty(), "Schema {} has empty SQL", schema.table);
            }
            SchemaSource::File(path) => {
                assert!(
                    !path.as_os_str().is_empty(),
                    "Schema {} has empty path",
                    schema.table
                );
            }
        }
    }
}

#[test]
fn test_files_extension_files_schema_sql_contains_table() {
    use systemprompt_extension::SchemaSource;

    let ext = FilesExtension;
    let schemas = ext.schemas();
    let files_schema = schemas.iter().find(|s| s.table == "files").unwrap();
    match &files_schema.sql {
        SchemaSource::Inline(sql) => {
            assert!(sql.contains("CREATE TABLE") || sql.contains("files"));
        }
        SchemaSource::File(_) => {}
    }
}

#[test]
fn test_files_extension_content_files_schema_sql_contains_table() {
    use systemprompt_extension::SchemaSource;

    let ext = FilesExtension;
    let schemas = ext.schemas();
    let schema = schemas.iter().find(|s| s.table == "content_files").unwrap();
    match &schema.sql {
        SchemaSource::Inline(sql) => {
            assert!(sql.contains("CREATE TABLE") || sql.contains("content_files"));
        }
        SchemaSource::File(_) => {}
    }
}

#[test]
fn test_files_extension_ai_image_analytics_schema() {
    let ext = FilesExtension;
    let schemas = ext.schemas();
    let schema = schemas
        .iter()
        .find(|s| s.table == "ai_image_analytics")
        .unwrap();
    assert_eq!(schema.table, "ai_image_analytics");
}
