use super::db_helper::pool_or_skip;
use systemprompt_database::SqlExecutor;

fn unique_table() -> String {
    format!("exec_test_{}", uuid::Uuid::new_v4().simple())
}

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
        "CREATE TRIGGER must terminate at its trailing semicolon, not swallow the next statement \
         — got {stmts:#?}"
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
fn unterminated_dollar_quote_returns_err() {
    let sql = "CREATE FUNCTION x() RETURNS void AS $body$ BEGIN RETURN; END;";
    let result = SqlExecutor::parse_sql_statements(sql);
    assert!(
        result.is_err(),
        "expected error for unterminated $body$ block, got {result:?}"
    );
}

#[test]
fn unterminated_single_quote_returns_err() {
    let sql = "INSERT INTO t VALUES ('oops";
    let result = SqlExecutor::parse_sql_statements(sql);
    assert!(
        result.is_err(),
        "expected error for unterminated string literal, got {result:?}"
    );
}

#[test]
fn create_function_preserves_empty_parameter_list() {
    let sql = "\
CREATE OR REPLACE FUNCTION update_timestamp_trigger()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 1, "got {stmts:#?}");
    assert!(
        stmts[0].contains("update_timestamp_trigger()"),
        "splitter must preserve the empty parameter list verbatim — Postgres rejects `CREATE \
         FUNCTION foo RETURNS …` without it. Got: {}",
        stmts[0]
    );
}

#[test]
fn block_comments_with_inner_semicolon_do_not_split() {
    let sql = "/* outer; /* nested; */ still inside; */ CREATE TABLE only_one (id INT);";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].to_uppercase().contains("CREATE TABLE"));
}

#[test]
fn leading_comments_are_carried_with_statement() {
    let sql = "\
-- a comment

-- another comment
CREATE TABLE only_one (id INT);
";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("CREATE TABLE only_one"));
}

#[test]
fn semicolon_only_block_emits_no_statements() {
    let stmts = SqlExecutor::parse_sql_statements(";;-- trailing\n").expect("parse ok");
    assert!(stmts.is_empty(), "got {stmts:#?}");
}

#[test]
fn statement_without_trailing_newline_is_still_emitted() {
    let sql = "CREATE TABLE bare (id INT);";
    let stmts = SqlExecutor::parse_sql_statements(sql).expect("parse ok");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].to_uppercase().starts_with("CREATE TABLE"));
    assert!(stmts[0].contains("bare"));
}

#[tokio::test]
async fn execute_statements_runs_batch_and_table_exists_tracks_it() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let table = unique_table();

    assert!(
        !SqlExecutor::table_exists(&db, &table)
            .await
            .expect("table_exists before"),
        "table must not exist before creation"
    );

    let ddl = format!("CREATE TABLE \"{table}\" (id INT PRIMARY KEY, label TEXT);");
    SqlExecutor::execute_statements(&db, &ddl)
        .await
        .expect("execute_statements");

    assert!(
        SqlExecutor::table_exists(&db, &table)
            .await
            .expect("table_exists after"),
        "table must exist after CREATE"
    );

    assert!(
        SqlExecutor::column_exists(&db, &table, "label")
            .await
            .expect("column_exists label"),
        "label column must exist"
    );
    assert!(
        !SqlExecutor::column_exists(&db, &table, "missing")
            .await
            .expect("column_exists missing"),
        "absent column must report false"
    );

    let drop = format!("DROP TABLE IF EXISTS \"{table}\";");
    SqlExecutor::execute_statements(&db, &drop)
        .await
        .expect("drop");
}

#[tokio::test]
async fn execute_query_returns_rows_and_columns() {
    let Some(db) = pool_or_skip().await else {
        return;
    };

    let result = SqlExecutor::execute_query(&db, "SELECT 1 AS one, 'x' AS letter")
        .await
        .expect("execute_query");

    assert_eq!(result.row_count, 1);
    assert!(result.columns.contains(&"one".to_string()));
    assert!(result.columns.contains(&"letter".to_string()));

    let row = &result.rows[0];
    assert_eq!(row.get("one").and_then(serde_json::Value::as_i64), Some(1));
    assert_eq!(
        row.get("letter").and_then(serde_json::Value::as_str),
        Some("x")
    );
}

#[tokio::test]
async fn execute_statements_parsed_runs_each_statement() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let table = unique_table();

    let provider = db.write();
    let sql = format!(
        "CREATE TABLE \"{table}\" (id INT PRIMARY KEY);\nINSERT INTO \"{table}\" (id) VALUES (1);\nINSERT INTO \"{table}\" (id) VALUES (2);"
    );
    SqlExecutor::execute_statements_parsed(provider, &sql)
        .await
        .expect("execute_statements_parsed");

    let count = SqlExecutor::execute_query(&db, &format!("SELECT COUNT(*) AS c FROM \"{table}\""))
        .await
        .expect("count query");
    assert_eq!(
        count.rows[0].get("c").and_then(serde_json::Value::as_i64),
        Some(2)
    );

    SqlExecutor::execute_statements(&db, &format!("DROP TABLE IF EXISTS \"{table}\";"))
        .await
        .expect("drop");
}

#[tokio::test]
async fn execute_file_reads_and_runs_sql() {
    use std::io::Write;
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let table = unique_table();

    let mut file = tempfile::NamedTempFile::new().expect("tempfile");
    write!(file, "CREATE TABLE \"{table}\" (id INT);").expect("write sql");
    let path = file.path().to_string_lossy().into_owned();

    SqlExecutor::execute_file(&db, &path)
        .await
        .expect("execute_file");
    assert!(
        SqlExecutor::table_exists(&db, &table)
            .await
            .expect("table_exists")
    );

    SqlExecutor::execute_statements(&db, &format!("DROP TABLE IF EXISTS \"{table}\";"))
        .await
        .expect("drop");
}

#[tokio::test]
async fn execute_file_missing_path_is_internal_error() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let err = SqlExecutor::execute_file(&db, "/nonexistent/path/to/file.sql")
        .await
        .expect_err("missing file must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("Failed to read SQL file"),
        "error must name the read failure, got: {msg}"
    );
}

#[tokio::test]
async fn execute_query_invalid_sql_is_internal_error() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let err = SqlExecutor::execute_query(&db, "SELECT * FROM definitely_not_a_table_xyz")
        .await
        .expect_err("bad query must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("Failed to execute query"),
        "error must name the query failure, got: {msg}"
    );
}

// --- terminal-state error arms ---
//
// `parse_sql_statements` walks the SQL as a small state machine; if it reaches
// the end of the input while still inside a quote, dollar-quote or block
// comment it must refuse the whole script rather than emit a truncated
// statement. Each arm names the construct that was left open.

#[test]
fn an_unterminated_single_quoted_literal_is_refused() {
    let err = SqlExecutor::parse_sql_statements("SELECT 'never closed")
        .expect_err("an open string literal must not yield a statement");
    assert!(
        err.to_string().contains("Unterminated string literal"),
        "got {err}"
    );
}

#[test]
fn an_unterminated_dollar_quoted_body_is_refused_and_names_its_tag() {
    let err =
        SqlExecutor::parse_sql_statements("CREATE FUNCTION f() RETURNS void AS $body$ BEGIN NULL;")
            .expect_err("an open dollar-quoted body must not yield a statement");
    let message = err.to_string();
    assert!(
        message.contains("Unterminated dollar-quoted string"),
        "got {message}"
    );
    assert!(
        message.contains("$body$"),
        "the error must name the tag that was left open, got {message}"
    );
}

#[test]
fn an_unterminated_block_comment_is_refused() {
    let err = SqlExecutor::parse_sql_statements("SELECT 1; /* still open")
        .expect_err("an open block comment must not yield a statement");
    assert!(
        err.to_string().contains("Unterminated block comment"),
        "got {err}"
    );
}

#[test]
fn a_doubled_quote_escapes_rather_than_closing_the_literal() {
    let stmts = SqlExecutor::parse_sql_statements("INSERT INTO t VALUES ('it''s fine; really');")
        .expect("a doubled quote is an escape, not a terminator");
    assert_eq!(
        stmts.len(),
        1,
        "the semicolon inside the literal must not split the statement: {stmts:?}"
    );
    assert!(stmts[0].contains("it''s fine; really"));
}

#[test]
fn a_lone_dollar_sign_is_ordinary_content_not_a_quote_opener() {
    let stmts = SqlExecutor::parse_sql_statements("SELECT 'a' || $ || 'b';")
        .expect("a `$` with no closing tag delimiter is just a character");
    assert_eq!(stmts.len(), 1, "got {stmts:?}");
}

#[test]
fn a_closed_dollar_quoted_body_keeps_its_semicolons() {
    let stmts = SqlExecutor::parse_sql_statements(
        "CREATE FUNCTION f() RETURNS void AS $$ BEGIN NULL; NULL; END; $$ LANGUAGE plpgsql; \
         SELECT 1;",
    )
    .expect("a closed dollar-quoted body parses");
    assert_eq!(
        stmts.len(),
        2,
        "only the semicolons outside the body split statements: {stmts:?}"
    );
    assert!(stmts[0].contains("BEGIN NULL; NULL; END;"));
}

#[test]
fn trailing_whitespace_and_comments_do_not_emit_an_empty_statement() {
    let stmts = SqlExecutor::parse_sql_statements("SELECT 1;\n\n-- trailing note\n   \n")
        .expect("trailing filler parses");
    assert_eq!(
        stmts.len(),
        1,
        "content-free trailing input must not become a statement: {stmts:?}"
    );
}
