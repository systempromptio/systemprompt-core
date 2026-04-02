use systemprompt_mcp::services::schema::{SchemaLoader, SchemaValidationMode, SchemaValidationReport};
use std::path::Path;

#[test]
fn validate_schema_syntax_valid_create_table() {
    let sql = "CREATE TABLE my_table (id INTEGER PRIMARY KEY, name TEXT)";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_schema_syntax_valid_create_table_if_not_exists() {
    let sql = "CREATE TABLE IF NOT EXISTS my_table (id INTEGER PRIMARY KEY)";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_schema_syntax_valid_comment_then_create() {
    let sql = "-- Migration script\nCREATE TABLE my_table (id INTEGER PRIMARY KEY)";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_schema_syntax_rejects_select() {
    let sql = "SELECT * FROM my_table";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_err());
}

#[test]
fn validate_schema_syntax_rejects_insert() {
    let sql = "INSERT INTO my_table VALUES (1, 'name')";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_err());
}

#[test]
fn validate_schema_syntax_rejects_drop_table() {
    let sql = "DROP TABLE my_table";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_err());
}

#[test]
fn validate_schema_syntax_rejects_alter_table() {
    let sql = "ALTER TABLE my_table ADD COLUMN new_col TEXT";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_err());
}

#[test]
fn validate_schema_syntax_valid_with_leading_whitespace() {
    let sql = "  CREATE TABLE my_table (id INTEGER PRIMARY KEY)";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_schema_syntax_valid_lowercase() {
    let sql = "create table my_table (id integer primary key)";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_schema_syntax_valid_mixed_case() {
    let sql = "Create Table my_table (id INTEGER PRIMARY KEY)";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_schema_syntax_rejects_empty_string() {
    let sql = "";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_err());
}

#[test]
fn validate_schema_syntax_rejects_whitespace_only() {
    let sql = "   \n\t  ";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_err());
}

#[test]
fn validate_schema_syntax_comment_without_create_table_fails() {
    let sql = "-- Just a comment\n-- Another comment";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("CREATE TABLE"));
}

#[test]
fn validate_schema_syntax_multiple_create_tables() {
    let sql = "CREATE TABLE t1 (id INTEGER);\nCREATE TABLE t2 (id INTEGER);";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_table_naming_valid_prefix_uppercase() {
    let sql = "CREATE TABLE MY_MODULE_USERS (id INTEGER PRIMARY KEY)";
    let result = SchemaLoader::validate_table_naming(sql, "MY-MODULE");
    assert!(result.is_ok());
}

#[test]
fn validate_table_naming_invalid_prefix() {
    let sql = "CREATE TABLE other_users (id INTEGER PRIMARY KEY)";
    let result = SchemaLoader::validate_table_naming(sql, "MY-MODULE");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("MY_MODULE"));
}

#[test]
fn validate_table_naming_no_create_table_statements() {
    let sql = "SELECT * FROM foo";
    let result = SchemaLoader::validate_table_naming(sql, "my-module");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("No CREATE TABLE"));
}

#[test]
fn validate_table_naming_multiple_tables_all_valid() {
    let sql = "CREATE TABLE MCP_SERVERS (id INTEGER);\nCREATE TABLE MCP_SESSIONS (id INTEGER);";
    let result = SchemaLoader::validate_table_naming(sql, "MCP");
    assert!(result.is_ok());
}

#[test]
fn validate_table_naming_multiple_tables_one_invalid() {
    let sql = "CREATE TABLE MCP_SERVERS (id INTEGER);\nCREATE TABLE OTHER_SESSIONS (id INTEGER);";
    let result = SchemaLoader::validate_table_naming(sql, "MCP");
    assert!(result.is_err());
}

#[test]
fn validate_table_naming_hyphen_to_underscore_conversion() {
    let sql = "CREATE TABLE MY_COOL_MODULE_TABLE (id INTEGER)";
    let result = SchemaLoader::validate_table_naming(sql, "MY-COOL-MODULE");
    assert!(result.is_ok());
}

#[test]
fn load_schema_file_nonexistent_path() {
    let result = SchemaLoader::load_schema_file(Path::new("/nonexistent/path"), "schema.sql");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not found"));
}

#[test]
fn list_schema_files_nonexistent_dir() {
    let result = SchemaLoader::list_schema_files(Path::new("/nonexistent/path"));
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn schema_validation_mode_from_string_strict() {
    let mode = SchemaValidationMode::from_string("strict");
    assert_eq!(mode, SchemaValidationMode::Strict);
}

#[test]
fn schema_validation_mode_from_string_skip() {
    let mode = SchemaValidationMode::from_string("skip");
    assert_eq!(mode, SchemaValidationMode::Skip);
}

#[test]
fn schema_validation_mode_from_string_auto_migrate() {
    let mode = SchemaValidationMode::from_string("auto_migrate");
    assert_eq!(mode, SchemaValidationMode::AutoMigrate);
}

#[test]
fn schema_validation_mode_from_string_default() {
    let mode = SchemaValidationMode::from_string("anything_else");
    assert_eq!(mode, SchemaValidationMode::AutoMigrate);
}

#[test]
fn schema_validation_mode_from_string_empty() {
    let mode = SchemaValidationMode::from_string("");
    assert_eq!(mode, SchemaValidationMode::AutoMigrate);
}

#[test]
fn schema_validation_mode_from_string_case_insensitive_strict() {
    let mode = SchemaValidationMode::from_string("STRICT");
    assert_eq!(mode, SchemaValidationMode::Strict);
}

#[test]
fn schema_validation_mode_from_string_case_insensitive_skip() {
    let mode = SchemaValidationMode::from_string("SKIP");
    assert_eq!(mode, SchemaValidationMode::Skip);
}

#[test]
fn schema_validation_mode_from_string_mixed_case_strict() {
    let mode = SchemaValidationMode::from_string("Strict");
    assert_eq!(mode, SchemaValidationMode::Strict);
}

#[test]
fn schema_validation_mode_clone_and_eq() {
    let mode = SchemaValidationMode::Strict;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn schema_validation_mode_debug() {
    let mode = SchemaValidationMode::AutoMigrate;
    let debug = format!("{:?}", mode);
    assert!(debug.contains("AutoMigrate"));
}

#[test]
fn schema_validation_report_new() {
    let report = SchemaValidationReport::new("test-service".to_string());
    assert_eq!(report.service_name, "test-service");
    assert_eq!(report.validated, 0);
    assert_eq!(report.created, 0);
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn schema_validation_report_merge() {
    let mut report1 = SchemaValidationReport::new("service-a".to_string());
    report1.validated = 3;
    report1.created = 1;
    report1.errors.push("error1".to_string());

    let mut report2 = SchemaValidationReport::new("service-b".to_string());
    report2.validated = 2;
    report2.created = 2;
    report2.warnings.push("warning1".to_string());

    report1.merge(report2);

    assert_eq!(report1.validated, 5);
    assert_eq!(report1.created, 3);
    assert_eq!(report1.errors.len(), 1);
    assert_eq!(report1.warnings.len(), 1);
}

#[test]
fn schema_validation_report_merge_empty() {
    let mut report1 = SchemaValidationReport::new("service-a".to_string());
    report1.validated = 3;

    let report2 = SchemaValidationReport::new("service-b".to_string());
    report1.merge(report2);

    assert_eq!(report1.validated, 3);
    assert_eq!(report1.created, 0);
    assert!(report1.errors.is_empty());
}

#[test]
fn schema_validation_report_merge_accumulates_errors() {
    let mut report1 = SchemaValidationReport::new("a".to_string());
    report1.errors.push("err1".to_string());
    report1.errors.push("err2".to_string());

    let mut report2 = SchemaValidationReport::new("b".to_string());
    report2.errors.push("err3".to_string());

    report1.merge(report2);
    assert_eq!(report1.errors.len(), 3);
    assert_eq!(report1.errors[2], "err3");
}

#[test]
fn schema_validation_report_merge_accumulates_warnings() {
    let mut report1 = SchemaValidationReport::new("a".to_string());
    report1.warnings.push("warn1".to_string());

    let mut report2 = SchemaValidationReport::new("b".to_string());
    report2.warnings.push("warn2".to_string());
    report2.warnings.push("warn3".to_string());

    report1.merge(report2);
    assert_eq!(report1.warnings.len(), 3);
}

#[test]
fn schema_validation_report_serialization() {
    let mut report = SchemaValidationReport::new("test-svc".to_string());
    report.validated = 5;
    report.created = 2;
    report.errors.push("some error".to_string());

    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("test-svc"));
    assert!(json.contains("some error"));

    let deserialized: SchemaValidationReport = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.service_name, "test-svc");
    assert_eq!(deserialized.validated, 5);
    assert_eq!(deserialized.created, 2);
    assert_eq!(deserialized.errors.len(), 1);
}

#[test]
fn schema_validation_report_debug() {
    let report = SchemaValidationReport::new("debug-test".to_string());
    let debug = format!("{:?}", report);
    assert!(debug.contains("SchemaValidationReport"));
    assert!(debug.contains("debug-test"));
}

#[test]
fn schema_validation_report_clone() {
    let mut report = SchemaValidationReport::new("clone-test".to_string());
    report.validated = 10;
    report.errors.push("cloned error".to_string());

    let cloned = report.clone();
    assert_eq!(cloned.service_name, "clone-test");
    assert_eq!(cloned.validated, 10);
    assert_eq!(cloned.errors.len(), 1);
}

#[test]
fn schema_validation_report_service_name_preserved_after_merge() {
    let mut report1 = SchemaValidationReport::new("original".to_string());
    let report2 = SchemaValidationReport::new("other".to_string());
    report1.merge(report2);
    assert_eq!(report1.service_name, "original");
}
