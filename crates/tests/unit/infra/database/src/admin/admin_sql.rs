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
fn readonly_strips_line_comments() {
    let sql = AdminSql::parse_readonly("-- drop everything\nSELECT 1").expect("must parse");
    assert!(sql.as_str().contains("SELECT 1"));
}

#[test]
fn readonly_strips_block_comments() {
    let sql = AdminSql::parse_readonly("/* DELETE FROM users */ SELECT 1").expect("must parse");
    assert_eq!(sql.as_str().trim(), "SELECT 1");
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
    assert!(matches!(result, Err(AdminSqlError::ForbiddenKeyword)));
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
