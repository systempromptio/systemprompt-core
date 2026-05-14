use systemprompt_database::services::schema_additivity::{
    DeclaredColumn, DeclaredTable, parse_declared_tables,
};

fn col(name: &str, type_text: &str) -> DeclaredColumn {
    DeclaredColumn {
        name: name.to_string(),
        type_text: type_text.to_string(),
    }
}

#[test]
fn parses_simple_create_table() {
    let sql = "CREATE TABLE logs (id TEXT, message TEXT);";
    let tables = parse_declared_tables(sql);
    assert_eq!(
        tables,
        vec![DeclaredTable {
            name: "logs".to_string(),
            columns: vec![col("id", "TEXT"), col("message", "TEXT")],
        }]
    );
}

#[test]
fn parses_if_not_exists_form() {
    let sql = "CREATE TABLE IF NOT EXISTS users (id BIGINT, name VARCHAR(255));";
    let tables = parse_declared_tables(sql);
    assert_eq!(
        tables,
        vec![DeclaredTable {
            name: "users".to_string(),
            columns: vec![col("id", "BIGINT"), col("name", "VARCHAR(255)")],
        }]
    );
}

#[test]
fn strips_column_constraints_from_type() {
    let sql = "\
CREATE TABLE logs (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    level VARCHAR(50) NOT NULL CHECK (level IN ('ERROR', 'INFO')),
    gateway_conversation_id VARCHAR(255)
);";
    let tables = parse_declared_tables(sql);
    let cols = &tables[0].columns;
    assert_eq!(cols[0], col("id", "TEXT"));
    assert_eq!(cols[1], col("timestamp", "TIMESTAMPTZ"));
    assert_eq!(cols[2], col("level", "VARCHAR(50)"));
    assert_eq!(cols[3], col("gateway_conversation_id", "VARCHAR(255)"));
}

#[test]
fn skips_table_level_constraints() {
    let sql = "\
CREATE TABLE events (
    id BIGINT,
    user_id VARCHAR(255),
    CONSTRAINT events_user_fkey FOREIGN KEY (user_id) REFERENCES users(id),
    PRIMARY KEY (id),
    UNIQUE (user_id, id),
    UNIQUE(user_id),
    CHECK (id > 0)
);";
    let tables = parse_declared_tables(sql);
    assert_eq!(tables[0].columns, vec![col("id", "BIGINT"), col("user_id", "VARCHAR(255)")]);
}

#[test]
fn handles_multi_word_types() {
    let sql = "CREATE TABLE m (val DOUBLE PRECISION NOT NULL);";
    let tables = parse_declared_tables(sql);
    assert_eq!(tables[0].columns, vec![col("val", "DOUBLE PRECISION")]);
}

#[test]
fn parses_multiple_tables_in_one_script() {
    let sql = "\
CREATE TABLE a (x INT);
CREATE TABLE IF NOT EXISTS b (y TEXT);
ALTER TABLE b ADD COLUMN z BOOLEAN;";
    let tables = parse_declared_tables(sql);
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].name, "a");
    assert_eq!(tables[1].name, "b");
}

#[test]
fn ignores_create_table_inside_dollar_quoted_function_body() {
    let sql = "\
CREATE OR REPLACE FUNCTION f() RETURNS void AS $$
BEGIN
    EXECUTE 'CREATE TABLE inner_fake (x INT)';
END;
$$ LANGUAGE plpgsql;
CREATE TABLE real_table (id INT);";
    let tables = parse_declared_tables(sql);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "real_table");
}

#[test]
fn ignores_create_table_inside_line_comment() {
    let sql = "\
-- CREATE TABLE commented_out (x INT);
CREATE TABLE real_one (id INT);";
    let tables = parse_declared_tables(sql);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "real_one");
}

#[test]
fn empty_script_returns_no_tables() {
    assert!(parse_declared_tables("").is_empty());
    assert!(parse_declared_tables("-- just a comment").is_empty());
}
