//! Tests for `admin::setup::ddl` — the bootstrap DDL statement builders and
//! their identifier/literal escaping.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::setup::ddl::{
    build_create_db_sql, build_create_user_sql, build_grant_sql, quote_ident, quote_literal,
};

#[test]
fn quote_ident_wraps_in_double_quotes() {
    assert_eq!(quote_ident("app_user"), "\"app_user\"");
}

#[test]
fn quote_ident_doubles_embedded_double_quotes() {
    assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
}

#[test]
fn quote_ident_handles_injection_attempt() {
    assert_eq!(
        quote_ident("x\"; DROP TABLE users; --"),
        "\"x\"\"; DROP TABLE users; --\""
    );
}

#[test]
fn quote_literal_wraps_in_single_quotes() {
    assert_eq!(quote_literal("hunter2"), "'hunter2'");
}

#[test]
fn quote_literal_doubles_embedded_single_quotes() {
    assert_eq!(quote_literal("o'brien"), "'o''brien'");
}

#[test]
fn quote_literal_preserves_double_quotes_verbatim() {
    assert_eq!(quote_literal("pa\"ss"), "'pa\"ss'");
}

#[test]
fn create_user_sql_quotes_user_and_password() {
    assert_eq!(
        build_create_user_sql("app", "s3cret"),
        "CREATE USER \"app\" WITH PASSWORD 's3cret'"
    );
}

#[test]
fn create_user_sql_escapes_quote_in_password() {
    assert_eq!(
        build_create_user_sql("app", "it's"),
        "CREATE USER \"app\" WITH PASSWORD 'it''s'"
    );
}

#[test]
fn create_user_sql_escapes_quote_in_user() {
    assert_eq!(
        build_create_user_sql("a\"b", "pw"),
        "CREATE USER \"a\"\"b\" WITH PASSWORD 'pw'"
    );
}

#[test]
fn create_db_sql_quotes_database_and_owner() {
    assert_eq!(
        build_create_db_sql("mydb", "app"),
        "CREATE DATABASE \"mydb\" OWNER \"app\""
    );
}

#[test]
fn create_db_sql_escapes_hyphenated_names() {
    assert_eq!(
        build_create_db_sql("systemprompt-web", "app-user"),
        "CREATE DATABASE \"systemprompt-web\" OWNER \"app-user\""
    );
}

#[test]
fn grant_sql_quotes_database_and_user() {
    assert_eq!(
        build_grant_sql("mydb", "app"),
        "GRANT ALL PRIVILEGES ON DATABASE \"mydb\" TO \"app\""
    );
}

#[test]
fn grant_sql_escapes_embedded_quotes() {
    assert_eq!(
        build_grant_sql("d\"b", "u\"ser"),
        "GRANT ALL PRIVILEGES ON DATABASE \"d\"\"b\" TO \"u\"\"ser\""
    );
}

#[test]
fn empty_password_still_quoted() {
    assert_eq!(
        build_create_user_sql("app", ""),
        "CREATE USER \"app\" WITH PASSWORD ''"
    );
}
