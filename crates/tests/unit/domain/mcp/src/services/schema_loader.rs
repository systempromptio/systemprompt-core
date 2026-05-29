use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_mcp::services::schema::SchemaLoader;

fn temp_dir_for(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mcp_schema_test_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn load_schema_file_reads_content() {
    let dir = temp_dir_for("reads_content");
    let file = dir.join("schema.sql");
    fs::write(&file, "CREATE TABLE test_tbl (id INTEGER)").expect("write");

    let result = SchemaLoader::load_schema_file(&dir, "schema.sql");
    cleanup(&dir);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("CREATE TABLE"));
}

#[test]
fn load_schema_file_missing_returns_err() {
    let result = SchemaLoader::load_schema_file(Path::new("/nonexistent/dir"), "nonexistent.sql");
    assert!(result.is_err());
}

#[test]
fn load_schema_file_empty_returns_err() {
    let dir = temp_dir_for("empty_file");
    let file = dir.join("empty.sql");
    fs::write(&file, "   \n\t  ").expect("write");

    let result = SchemaLoader::load_schema_file(&dir, "empty.sql");
    cleanup(&dir);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.to_lowercase().contains("empty"));
}

#[test]
fn load_schema_file_with_leading_comment_ok() {
    let dir = temp_dir_for("leading_comment");
    let file = dir.join("commented.sql");
    fs::write(&file, "-- setup\nCREATE TABLE test_a (id INTEGER)").expect("write");

    let result = SchemaLoader::load_schema_file(&dir, "commented.sql");
    cleanup(&dir);
    assert!(result.is_ok());
}

#[test]
fn list_schema_files_returns_only_sql_files() {
    let dir = temp_dir_for("list_sql");
    let schema_dir = dir.join("schema");
    fs::create_dir_all(&schema_dir).expect("create schema dir");
    fs::write(schema_dir.join("001_init.sql"), "CREATE TABLE x (id INTEGER)").expect("write");
    fs::write(schema_dir.join("notes.txt"), "not sql").expect("write");
    fs::write(schema_dir.join("002_more.sql"), "CREATE TABLE y (id INTEGER)").expect("write");

    let files = SchemaLoader::list_schema_files(&dir).expect("list");
    cleanup(&dir);
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f.extension().and_then(|e| e.to_str()) == Some("sql")));
}

#[test]
fn list_schema_files_nonexistent_dir_empty() {
    let result = SchemaLoader::list_schema_files(Path::new("/nonexistent/abc/xyz"));
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn list_schema_files_empty_schema_dir() {
    let dir = temp_dir_for("empty_schema");
    let schema_dir = dir.join("schema");
    fs::create_dir_all(&schema_dir).expect("create");

    let files = SchemaLoader::list_schema_files(&dir).expect("list");
    cleanup(&dir);
    assert!(files.is_empty());
}

#[test]
fn validate_schema_syntax_create_table_if_not_exists_ok() {
    let sql = "CREATE TABLE IF NOT EXISTS some_table (id TEXT PRIMARY KEY)";
    SchemaLoader::validate_schema_syntax(sql).expect("should be valid");
}

#[test]
fn validate_schema_syntax_update_rejected() {
    let sql = "UPDATE users SET name='x'";
    assert!(SchemaLoader::validate_schema_syntax(sql).is_err());
}

#[test]
fn validate_schema_syntax_delete_rejected() {
    let sql = "DELETE FROM users WHERE id=1";
    assert!(SchemaLoader::validate_schema_syntax(sql).is_err());
}

#[test]
fn validate_schema_syntax_empty_rejected() {
    assert!(SchemaLoader::validate_schema_syntax("").is_err());
}

#[test]
fn validate_schema_syntax_whitespace_only_rejected() {
    assert!(SchemaLoader::validate_schema_syntax("   \n\t  ").is_err());
}

#[test]
fn validate_table_naming_if_not_exists_variant() {
    // extract_table_names uppercases the SQL; the module_prefix stays lowercase.
    // "MCP_SESSIONS".starts_with("mcp") = false → validation fails.
    // Use uppercase module name to match:
    let sql = "CREATE TABLE IF NOT EXISTS MCP_SESSIONS (id TEXT PRIMARY KEY)";
    let result = SchemaLoader::validate_table_naming(sql, "MCP");
    assert!(result.is_ok());
}

#[test]
fn validate_table_naming_multiple_tables_same_prefix() {
    let sql = "CREATE TABLE MCP_SESSIONS (id INTEGER);\nCREATE TABLE MCP_TOOLS (id INTEGER);";
    let result = SchemaLoader::validate_table_naming(sql, "MCP");
    assert!(result.is_ok());
}

#[test]
fn validate_table_naming_hyphen_converted_to_underscore() {
    // module_prefix = "MY_SVC"; SQL uppercased table = "MY_SVC_TABLE".
    // "MY_SVC_TABLE".starts_with("MY_SVC") → ok.
    let sql = "CREATE TABLE MY_SVC_TABLE (id INTEGER)";
    let result = SchemaLoader::validate_table_naming(sql, "MY-SVC");
    assert!(result.is_ok());
}

#[test]
fn validate_table_naming_no_tables_fails() {
    let sql = "-- no tables here";
    let result = SchemaLoader::validate_table_naming(sql, "mcp");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("No CREATE TABLE") || msg.contains("No create table"));
}

#[test]
fn validate_table_naming_one_invalid_among_valid_fails() {
    let sql = "CREATE TABLE MCP_OK (id INTEGER);\nCREATE TABLE OTHER_BAD (id INTEGER);";
    let result = SchemaLoader::validate_table_naming(sql, "MCP");
    assert!(result.is_err());
}

#[test]
fn load_schema_file_preserves_content_exactly() {
    let dir = temp_dir_for("exact_content");
    let sql = "CREATE TABLE my_table (\n    id INTEGER PRIMARY KEY,\n    name TEXT NOT NULL\n);\n";
    let file = dir.join("full.sql");
    fs::write(&file, sql).expect("write");

    let content = SchemaLoader::load_schema_file(&dir, "full.sql").expect("read");
    cleanup(&dir);
    assert_eq!(content, sql);
}

#[test]
fn validate_schema_syntax_backtick_quoted_table() {
    let sql = "CREATE TABLE `my_table` (id INTEGER)";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}

#[test]
fn validate_schema_syntax_comment_with_create_table_inside_passes_because_string_match() {
    // The validator uses string search (contains), not SQL parsing.
    // A comment line containing "CREATE TABLE" satisfies the contains() check.
    let sql = "-- CREATE TABLE reference\nSELECT 1";
    let result = SchemaLoader::validate_schema_syntax(sql);
    assert!(result.is_ok());
}
