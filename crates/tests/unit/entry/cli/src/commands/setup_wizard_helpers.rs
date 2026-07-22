//! Tests for setup-wizard helpers: overwrite gating, database info
//! projection, and the cancelled-setup output.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::setup::SetupArgs;
use systemprompt_cli::admin::setup::common::PostgresConfig;
use systemprompt_cli::admin::setup::wizard::{build_cancelled, database_info, should_write};

fn args() -> SetupArgs {
    SetupArgs {
        environment: Some("covtest".to_owned()),
        docker: true,
        db_host: "db.example.com".to_owned(),
        db_port: 5433,
        db_user: None,
        db_password: None,
        db_name: None,
        gemini_key: None,
        anthropic_key: Some("sk-ant".to_owned()),
        openai_key: None,
        github_token: None,
        default_provider: None,
        migrate: false,
        no_migrate: true,
        dry_run: false,
        yes: true,
        force: false,
    }
}

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

#[test]
fn should_write_allows_missing_paths_and_force() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("profile.yaml");
    assert!(should_write(&missing, false, &cfg()));

    let existing = dir.path().join("existing.yaml");
    std::fs::write(&existing, "name: x").unwrap();
    assert!(!should_write(&existing, false, &cfg()));
    assert!(should_write(&existing, true, &cfg()));
}

#[test]
fn database_info_projects_connection_details() {
    let pg = PostgresConfig {
        host: "db.example.com".to_owned(),
        port: 5433,
        user: "systemprompt_covtest".to_owned(),
        password: "secret".to_owned(),
        database: "covtest_db".to_owned(),
    };
    let info = database_info(&pg, "connected", true);
    assert_eq!(info.host, "db.example.com");
    assert_eq!(info.port, 5433);
    assert_eq!(info.name, "covtest_db");
    assert_eq!(info.user, "systemprompt_covtest");
    assert_eq!(info.connection_status, "connected");
    assert!(info.docker);
}

#[test]
fn build_cancelled_reports_effective_defaults_and_key_presence() {
    let output = build_cancelled(&args(), "covtest", &cfg());
    let json = serde_json::to_string(&serde_json::to_value(output.artifact()).unwrap()).unwrap();
    assert!(json.contains("cancelled"), "artifact: {json}");
    assert!(json.contains("Setup cancelled by user"), "artifact: {json}");
    assert!(json.contains("systemprompt_covtest"), "artifact: {json}");
    assert!(json.contains("anthropic"), "artifact: {json}");
    assert!(json.contains("db.example.com"), "artifact: {json}");
}
