use systemprompt_database::SqlExecutor;

#[test]
fn single_create_table_emits_one_statement() {
    let sql = "CREATE TABLE foo (id INT);";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].to_uppercase().starts_with("CREATE TABLE"));
    assert!(stmts[0].contains("foo"));
}

#[test]
fn create_trigger_does_not_swallow_following_statements() {
    let sql = "\
CREATE TRIGGER update_user_contexts_updated_at
BEFORE UPDATE ON user_contexts
FOR EACH ROW EXECUTE FUNCTION update_timestamp_trigger();
CREATE TABLE next_thing (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(
        stmts.len(),
        2,
        "CREATE TRIGGER must terminate at its trailing semicolon, not swallow the next statement — got {stmts:#?}"
    );
    assert!(stmts[0].to_uppercase().contains("CREATE TRIGGER"));
    assert!(stmts[0].to_uppercase().contains("EXECUTE FUNCTION"));
    assert!(stmts[1].to_uppercase().contains("CREATE TABLE"));
    assert!(stmts[1].contains("next_thing"));
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
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 2, "got {stmts:#?}");
    assert!(stmts[0].contains("PERFORM 1"));
    assert!(stmts[0].contains("PERFORM 2"));
    assert!(stmts[1].to_uppercase().contains("CREATE TABLE"));
    assert!(stmts[1].contains("trailing"));
}

#[test]
fn named_dollar_quoted_function_is_one_statement() {
    let sql = "\
CREATE OR REPLACE FUNCTION sample_named()
RETURNS void AS $body$
BEGIN
    PERFORM 1;
    PERFORM 2;
END;
$body$ LANGUAGE plpgsql;
CREATE TABLE after_named (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 2, "got {stmts:#?}");
    assert!(stmts[0].contains("PERFORM 1"));
    assert!(stmts[1].contains("after_named"));
}

#[test]
fn apostrophe_quoted_function_body_is_one_statement() {
    let sql = "\
CREATE FUNCTION legacy_apos() RETURNS void AS 'BEGIN PERFORM 1; PERFORM 2; END;' LANGUAGE plpgsql;
CREATE TABLE after_apos (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 2, "got {stmts:#?}");
    assert!(stmts[1].contains("after_apos"));
}

#[test]
fn malformed_sql_returns_err() {
    let sql = "CREATE TABLE foo (";
    let result = SqlExecutor::parse_sql_statements(sql);
    assert!(
        result.is_err(),
        "expected parse error for unterminated CREATE TABLE, got {result:?}"
    );
}

#[test]
fn comments_and_blank_lines_are_skipped() {
    let sql = "\
-- a comment

-- another comment
CREATE TABLE only_one (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].to_uppercase().starts_with("CREATE TABLE"));
    assert!(stmts[0].contains("only_one"));
    assert!(!stmts[0].contains("--"));
}

#[test]
fn statement_without_trailing_newline_is_still_emitted() {
    let sql = "CREATE TABLE bare (id INT);";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].to_uppercase().starts_with("CREATE TABLE"));
    assert!(stmts[0].contains("bare"));
}
