//! Bootstrap DDL statement builders for `PostgreSQL` provisioning.
//!
//! `CREATE USER` / `CREATE DATABASE` / `GRANT` run before the target database
//! exists, so they cannot bind parameters; identifiers and literals are
//! escaped here instead. Statement execution stays in the setup wizard.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[must_use]
pub fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[must_use]
pub fn build_create_user_sql(user: &str, password: &str) -> String {
    format!(
        "CREATE USER {} WITH PASSWORD {}",
        quote_ident(user),
        quote_literal(password)
    )
}

#[must_use]
pub fn build_create_db_sql(database: &str, owner: &str) -> String {
    format!(
        "CREATE DATABASE {} OWNER {}",
        quote_ident(database),
        quote_ident(owner)
    )
}

#[must_use]
pub fn build_grant_sql(database: &str, user: &str) -> String {
    format!(
        "GRANT ALL PRIVILEGES ON DATABASE {} TO {}",
        quote_ident(database),
        quote_ident(user)
    )
}
