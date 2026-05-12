use systemprompt_database::SqlExecutor;

#[test]
fn single_create_table_emits_one_statement() {
    let sql = "CREATE TABLE foo (id INT);";
    let stmts = SqlExecutor::parse_sql_statements(sql);
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].starts_with("CREATE TABLE foo"));
}

#[test]
fn create_trigger_does_not_swallow_following_statements() {
    let sql = "\
CREATE TRIGGER update_user_contexts_updated_at
BEFORE UPDATE ON user_contexts
FOR EACH ROW EXECUTE FUNCTION update_timestamp_trigger();
CREATE TABLE next_thing (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql);
    assert_eq!(
        stmts.len(),
        2,
        "CREATE TRIGGER must terminate at its trailing semicolon, not wait for END;/LANGUAGE plpgsql; — got {stmts:#?}"
    );
    assert!(stmts[0].contains("CREATE TRIGGER"));
    assert!(stmts[0].contains("EXECUTE FUNCTION"));
    assert!(stmts[1].contains("CREATE TABLE next_thing"));
}

#[test]
fn dollar_quoted_function_keeps_inner_semicolons() {
    let sql = "\
CREATE OR REPLACE FUNCTION sample()
RETURNS void AS $$
BEGIN
    PERFORM 1;
    PERFORM 2;
END;
$$ LANGUAGE plpgsql;
CREATE TABLE trailing (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql);
    assert_eq!(stmts.len(), 2, "got {stmts:#?}");
    assert!(stmts[0].contains("PERFORM 1;"));
    assert!(stmts[0].contains("PERFORM 2;"));
    assert!(stmts[1].contains("CREATE TABLE trailing"));
}

#[test]
fn comments_and_blank_lines_are_skipped() {
    let sql = "\
-- a comment

-- another comment
CREATE TABLE only (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql);
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].starts_with("CREATE TABLE only"));
    assert!(!stmts[0].contains("--"));
}

#[test]
fn statement_without_trailing_newline_is_still_emitted() {
    let sql = "CREATE TABLE bare (id INT);";
    let stmts = SqlExecutor::parse_sql_statements(sql);
    assert_eq!(stmts.len(), 1);
    assert_eq!(stmts[0], "CREATE TABLE bare (id INT);");
}
