//! Tests for tenant-provisioning database connection info parsing.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::tenant::create::progress::DatabaseConnectionInfo;

#[test]
fn full_url_parses_every_component() {
    let info =
        DatabaseConnectionInfo::parse("postgres://admin:sekret@db.example.com:6432/tenant_db")
            .unwrap();

    assert_eq!(info.host, "db.example.com");
    assert_eq!(info.port, 6432);
    assert_eq!(info.database, "tenant_db");
    assert_eq!(info.username, "admin");
    assert_eq!(info.password, "sekret");
    assert_eq!(
        info.psql_command(),
        "PGPASSWORD='sekret' psql -h db.example.com -p 6432 -U admin -d tenant_db"
    );
}

#[test]
fn defaults_apply_for_missing_port_and_password() {
    let info = DatabaseConnectionInfo::parse("postgres://admin@db.example.com/tenant_db").unwrap();

    assert_eq!(info.port, 5432);
    assert_eq!(info.password, "********");
}

#[test]
fn invalid_url_yields_none() {
    assert!(DatabaseConnectionInfo::parse("not a url").is_none());
}
