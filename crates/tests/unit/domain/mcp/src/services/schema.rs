use std::path::Path;
use systemprompt_mcp::services::schema::SchemaLoader;

#[test]
fn validate_schema_syntax_valid_create_table() {
    let sql = "CREATE TABLE my_table (id INTEGER PRIMARY KEY, name TEXT)";
    SchemaLoader::validate_schema_syntax(sql).expect("create table is valid syntax");
}

#[test]
fn validate_schema_syntax_valid_create_table_if_not_exists() {
    let sql = "CREATE TABLE IF NOT EXISTS my_table (id INTEGER PRIMARY KEY)";
    SchemaLoader::validate_schema_syntax(sql).expect("create table if not exists is valid");
}

#[test]
fn validate_schema_syntax_valid_comment_then_create() {
    let sql = "-- Migration script\nCREATE TABLE my_table (id INTEGER PRIMARY KEY)";
    SchemaLoader::validate_schema_syntax(sql).expect("comment then create table is valid");
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
    SchemaLoader::validate_schema_syntax(sql).expect("leading whitespace is valid");
}

#[test]
fn validate_schema_syntax_valid_lowercase() {
    let sql = "create table my_table (id integer primary key)";
    SchemaLoader::validate_schema_syntax(sql).expect("lowercase create table is valid");
}

#[test]
fn validate_schema_syntax_valid_mixed_case() {
    let sql = "Create Table my_table (id INTEGER PRIMARY KEY)";
    SchemaLoader::validate_schema_syntax(sql).expect("mixed case create table is valid");
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
    SchemaLoader::validate_schema_syntax(sql).expect("multiple create tables are valid");
}

#[test]
fn validate_table_naming_valid_prefix_uppercase() {
    let sql = "CREATE TABLE MY_MODULE_USERS (id INTEGER PRIMARY KEY)";
    SchemaLoader::validate_table_naming(sql, "MY-MODULE").expect("matching prefix validates");
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
    SchemaLoader::validate_table_naming(sql, "MCP").expect("all tables share prefix");
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
    SchemaLoader::validate_table_naming(sql, "MY-COOL-MODULE")
        .expect("hyphenated module name converts to underscore prefix");
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
