//! Unit tests for AdminSql validation

use systemprompt_database::{AdminSql, AdminSqlError};

#[test]
fn readonly_accepts_plain_select() {
    let sql = AdminSql::parse_readonly("SELECT * FROM users").expect("select must parse");
    assert_eq!(sql.as_str(), "SELECT * FROM users");
}

#[test]
fn readonly_accepts_with_cte() {
    assert!(AdminSql::parse_readonly("WITH t AS (SELECT 1) SELECT * FROM t").is_ok());
}

#[test]
fn readonly_accepts_explain() {
    assert!(AdminSql::parse_readonly("EXPLAIN SELECT 1").is_ok());
}

#[test]
fn readonly_accepts_show() {
    assert!(AdminSql::parse_readonly("SHOW TIMEZONE").is_ok());
}

#[test]
fn readonly_strips_trailing_semicolon() {
    let sql = AdminSql::parse_readonly("SELECT 1;").expect("trailing semicolon ok");
    assert_eq!(sql.as_str(), "SELECT 1");
}

#[test]
fn readonly_ignores_line_comments() {
    let sql = AdminSql::parse_readonly("-- drop everything\nSELECT 1").expect("must parse");
    assert!(sql.as_str().contains("SELECT 1"));
}

#[test]
fn readonly_ignores_write_keywords_inside_block_comments() {
    let sql = AdminSql::parse_readonly("/* DELETE FROM users */ SELECT 1").expect("must parse");
    assert!(sql.as_str().ends_with("SELECT 1"));
}

#[test]
fn readonly_rejects_empty() {
    assert!(matches!(
        AdminSql::parse_readonly(""),
        Err(AdminSqlError::Empty)
    ));
}

#[test]
fn readonly_rejects_whitespace_only() {
    assert!(matches!(
        AdminSql::parse_readonly("   \n\t  "),
        Err(AdminSqlError::Empty)
    ));
}

#[test]
fn readonly_rejects_plain_delete() {
    assert!(matches!(
        AdminSql::parse_readonly("DELETE FROM users"),
        Err(AdminSqlError::NotReadOnly)
    ));
}

#[test]
fn readonly_rejects_plain_update() {
    assert!(matches!(
        AdminSql::parse_readonly("UPDATE users SET name = 'x'"),
        Err(AdminSqlError::NotReadOnly)
    ));
}

#[test]
fn readonly_rejects_plain_drop() {
    assert!(matches!(
        AdminSql::parse_readonly("DROP TABLE users"),
        Err(AdminSqlError::NotReadOnly)
    ));
}

#[test]
fn readonly_rejects_multi_statement() {
    assert!(matches!(
        AdminSql::parse_readonly("SELECT 1; DROP TABLE users"),
        Err(AdminSqlError::MultipleStatements)
    ));
}

#[test]
fn readonly_rejects_smuggled_drop_in_cte() {
    let result = AdminSql::parse_readonly("WITH t AS (SELECT 1) DROP TABLE users");
    assert!(matches!(result, Err(AdminSqlError::Parse(_))), "{result:?}");
}

#[test]
fn readonly_rejects_a_data_modifying_cte() {
    for smuggled in [
        "WITH d AS (DELETE FROM users RETURNING *) SELECT * FROM d",
        "WITH u AS (UPDATE users SET name = 'x' RETURNING id) SELECT count(*) FROM u",
        "WITH i AS (INSERT INTO users (id) VALUES (1) RETURNING id) SELECT * FROM i",
        "SELECT * FROM (WITH d AS (DELETE FROM users RETURNING *) SELECT * FROM d) AS s",
    ] {
        let result = AdminSql::parse_readonly(smuggled);
        assert!(
            matches!(result, Err(AdminSqlError::WriteInReadOnly)),
            "{smuggled}: {result:?}"
        );
    }
}

#[test]
fn readonly_rejects_dml_hidden_behind_whitespace_and_parentheses() {
    for raw in [
        "(DELETE FROM users)",
        "\n\tDELETE FROM users",
        "EXPLAIN DELETE FROM users",
    ] {
        let result = AdminSql::parse_readonly(raw);
        assert!(result.is_err(), "{raw}: {result:?}");
    }
}

#[test]
fn readonly_accepts_a_select_with_a_trailing_comment() {
    let sql = AdminSql::parse_readonly("select 1;\n-- x").expect("comment after terminator");
    assert_eq!(sql.as_str(), "select 1");
}

#[test]
fn readonly_rejects_comment_smuggled_drop() {
    assert!(matches!(
        AdminSql::parse_readonly("SELECT 1 /* fine */; DROP TABLE users"),
        Err(AdminSqlError::MultipleStatements)
    ));
}

#[test]
fn unrestricted_accepts_delete() {
    assert!(AdminSql::parse_unrestricted("DELETE FROM users WHERE id = 1").is_ok());
}

#[test]
fn unrestricted_rejects_multi_statement() {
    assert!(matches!(
        AdminSql::parse_unrestricted("UPDATE a SET x=1; UPDATE b SET y=2"),
        Err(AdminSqlError::MultipleStatements)
    ));
}

// Every case opens with `WITH`, so only the statement-node walk can refuse it.
#[test]
fn every_forbidden_statement_is_refused_behind_an_allowed_prefix() {
    for keyword in [
        "DROP TABLE users",
        "DELETE FROM users",
        "INSERT INTO users VALUES (1)",
        "UPDATE users SET name = 'x'",
        "ALTER TABLE users ADD COLUMN x INT",
        "CREATE TABLE evil (id INT)",
        "TRUNCATE users",
        "GRANT ALL ON users TO evil",
        "REVOKE ALL ON users FROM admin",
        "COPY users TO '/tmp/out'",
        "VACUUM users",
        "CALL some_procedure()",
        "LOCK TABLE users",
        "SET ROLE postgres",
        "RESET ROLE",
        "ALTER TABLE users RENAME TO gone",
    ] {
        let smuggled = format!("WITH t AS (SELECT 1) {keyword}");

        assert!(
            AdminSql::parse_readonly(&smuggled).is_err(),
            "a read-only query accepted {smuggled:?}"
        );
    }
}

// Why: the guard must not refuse ordinary reads. A keyword appearing inside an
// identifier or a string literal is not a statement, and refusing those would
// make the console unusable for the tables it exists to inspect.
#[test]
fn ordinary_reads_are_not_refused_by_the_statement_walk() {
    for benign in [
        "SELECT * FROM dropped_sessions",
        "SELECT created_at FROM users",
        "SELECT * FROM update_log",
        "WITH t AS (SELECT 1) SELECT * FROM t",
        "SELECT 'DELETE FROM users' AS literal",
        "SELECT * FROM users WHERE name = 'drop'",
    ] {
        assert!(
            AdminSql::parse_readonly(benign).is_ok(),
            "a legitimate read was refused: {benign:?}"
        );
    }
}
