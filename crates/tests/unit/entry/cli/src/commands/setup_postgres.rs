//! Tests for the setup wizard's PostgreSQL provisioning flows, driven with
//! `ScriptedPrompter` and a scratch database resolved from `DATABASE_URL`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::setup::SetupArgs;
use systemprompt_cli::admin::setup::common::{
    PostgresConfig, detect_postgresql, generate_password, test_connection,
};
use systemprompt_cli::admin::setup::docker_compose::create_compose_files_if_missing;
use systemprompt_cli::admin::setup::postgres::{setup_interactive, setup_non_interactive};
use systemprompt_cli::interactive::ScriptedPrompter;

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn args() -> SetupArgs {
    SetupArgs {
        environment: Some("covtest".to_owned()),
        docker: false,
        db_host: "127.0.0.1".to_owned(),
        db_port: 5432,
        db_user: None,
        db_password: None,
        db_name: None,
        gemini_key: None,
        anthropic_key: None,
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

struct DbUrl {
    user: String,
    password: String,
    host: String,
    port: u16,
    database: String,
}

fn db_url() -> Option<DbUrl> {
    let raw = std::env::var("DATABASE_URL").ok()?;
    let url = url::Url::parse(&raw).ok()?;
    Some(DbUrl {
        user: url.username().to_owned(),
        password: url.password()?.to_owned(),
        host: url.host_str()?.to_owned(),
        port: url.port().unwrap_or(5432),
        database: url.path().trim_start_matches('/').to_owned(),
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime")
}

#[test]
fn generate_password_is_16_alphanumeric() {
    let pw = generate_password();
    assert_eq!(pw.len(), 16);
    assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn detect_postgresql_false_for_closed_port() {
    assert!(!detect_postgresql("127.0.0.1", 9));
}

#[test]
fn detect_postgresql_false_for_unresolvable_host() {
    assert!(!detect_postgresql("no-such-host.invalid", 5432));
}

#[test]
fn database_url_formats_credentials() {
    let config = PostgresConfig {
        host: "localhost".to_owned(),
        port: 5433,
        user: "u".to_owned(),
        password: "p".to_owned(),
        database: "d".to_owned(),
    };
    assert_eq!(config.database_url(), "postgres://u:p@localhost:5433/d");
}

#[test]
fn test_connection_fails_for_bad_credentials() {
    let config = PostgresConfig {
        host: "127.0.0.1".to_owned(),
        port: 9,
        user: "nobody".to_owned(),
        password: "nothing".to_owned(),
        database: "nowhere".to_owned(),
    };
    assert!(!runtime().block_on(test_connection(&config)));
}

#[test]
fn non_interactive_with_reachable_database() {
    let Some(db) = db_url() else { return };
    let mut setup_args = args();
    setup_args.db_host = db.host.clone();
    setup_args.db_port = db.port;
    setup_args.db_user = Some(db.user.clone());
    setup_args.db_password = Some(db.password.clone());
    setup_args.db_name = Some(db.database.clone());

    let config = runtime()
        .block_on(setup_non_interactive(
            &setup_args,
            "covtest",
            &CliConfig::default(),
        ))
        .expect("non-interactive setup succeeds");
    assert_eq!(config.user, db.user);
    assert_eq!(config.database, db.database);
}

#[test]
fn non_interactive_with_unreachable_database_still_returns_config() {
    let mut setup_args = args();
    setup_args.db_host = "127.0.0.1".to_owned();
    setup_args.db_port = 9;

    let config = runtime()
        .block_on(setup_non_interactive(
            &setup_args,
            "covtest",
            &CliConfig::default(),
        ))
        .expect("unreachable database is a warning, not an error");
    assert_eq!(config.user, "systemprompt_covtest");
    assert_eq!(config.database, "systemprompt_covtest");
    assert_eq!(config.password.len(), 16);
}

#[test]
fn interactive_existing_connection_succeeds() {
    let Some(db) = db_url() else { return };
    let mut setup_args = args();
    setup_args.db_password = Some(db.password.clone());

    let prompter = scripted(&[
        "0",
        db.host.as_str(),
        &db.port.to_string(),
        db.user.as_str(),
        db.database.as_str(),
    ]);
    let config = runtime()
        .block_on(setup_interactive(
            &setup_args,
            &prompter,
            "covtest",
            &CliConfig::default(),
        ))
        .expect("interactive setup against live database succeeds");
    assert_eq!(config.host, db.host);
    assert_eq!(config.user, db.user);
}

#[test]
fn interactive_invalid_port_is_error() {
    let prompter = scripted(&["0", "127.0.0.1", "not-a-port"]);
    let err = runtime()
        .block_on(setup_interactive(
            &args(),
            &prompter,
            "covtest",
            &CliConfig::default(),
        ))
        .unwrap_err();
    assert!(err.to_string().contains("Invalid PostgreSQL port"));
}

#[test]
fn interactive_unreachable_declined_bails() {
    let prompter = scripted(&["0", "127.0.0.1", "9", "n"]);
    let err = runtime()
        .block_on(setup_interactive(
            &args(),
            &prompter,
            "covtest",
            &CliConfig::default(),
        ))
        .unwrap_err();
    assert!(err.to_string().contains("not reachable"));
}

#[test]
fn interactive_invalid_selection_is_error() {
    let prompter = scripted(&["7"]);
    let result = runtime().block_on(setup_interactive(
        &args(),
        &prompter,
        "covtest",
        &CliConfig::default(),
    ));
    assert!(result.is_err());
}

#[test]
fn interactive_generated_password_and_skipped_creation() {
    let Some(db) = db_url() else { return };
    let prompter = scripted(&[
        "0",
        db.host.as_str(),
        &db.port.to_string(),
        "cov_missing_user",
        "y",
        "cov_missing_db",
        "n",
    ]);
    let config = runtime()
        .block_on(setup_interactive(
            &args(),
            &prompter,
            "covtest",
            &CliConfig::default(),
        ))
        .expect("declining creation still returns the config");
    assert_eq!(config.user, "cov_missing_user");
    assert_eq!(config.database, "cov_missing_db");
}

#[test]
fn interactive_empty_manual_password_is_error() {
    let Some(db) = db_url() else { return };
    let prompter = scripted(&[
        "0",
        db.host.as_str(),
        &db.port.to_string(),
        "someuser",
        "n",
        "",
    ]);
    let err = runtime()
        .block_on(setup_interactive(
            &args(),
            &prompter,
            "covtest",
            &CliConfig::default(),
        ))
        .unwrap_err();
    assert!(err.to_string().contains("Password is required"));
}

#[test]
fn interactive_creates_database_and_user_via_superuser() {
    let Some(db) = db_url() else { return };
    let scratch_user = "cov_setup_role";
    let scratch_db = "cov_setup_db";
    runtime().block_on(async {
        let admin_url = format!(
            "postgres://{}:{}@{}:{}/postgres",
            db.user, db.password, db.host, db.port
        );
        let pool = sqlx::PgPool::connect(&admin_url).await.expect("connect");
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS {scratch_db}"
        )))
        .execute(&pool)
        .await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP ROLE IF EXISTS {scratch_user}"
        )))
        .execute(&pool)
        .await;
    });

    let mut setup_args = args();
    setup_args.db_password = Some("cov_setup_pw_123".to_owned());
    let prompter = scripted(&[
        "0",
        db.host.as_str(),
        &db.port.to_string(),
        scratch_user,
        scratch_db,
        "y",
        db.user.as_str(),
        db.password.as_str(),
    ]);
    let config = runtime()
        .block_on(setup_interactive(
            &setup_args,
            &prompter,
            "covtest",
            &CliConfig::default(),
        ))
        .expect("superuser-driven creation succeeds");
    assert_eq!(config.database, scratch_db);
    assert!(runtime().block_on(test_connection(&config)));

    runtime().block_on(async {
        let admin_url = format!(
            "postgres://{}:{}@{}:{}/postgres",
            db.user, db.password, db.host, db.port
        );
        let pool = sqlx::PgPool::connect(&admin_url).await.expect("connect");
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS {scratch_db} WITH (FORCE)"
        )))
        .execute(&pool)
        .await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP ROLE IF EXISTS {scratch_user}"
        )))
        .execute(&pool)
        .await;
    });
}

#[test]
fn interactive_superuser_empty_password_is_error() {
    let Some(db) = db_url() else { return };
    let prompter = scripted(&[
        "0",
        db.host.as_str(),
        &db.port.to_string(),
        "cov_nouser",
        "y",
        "cov_nodb",
        "y",
        "postgres",
        "",
    ]);
    let err = runtime()
        .block_on(setup_interactive(
            &args(),
            &prompter,
            "covtest",
            &CliConfig::default(),
        ))
        .unwrap_err();
    assert!(err.to_string().contains("Superuser password is required"));
}

#[test]
fn compose_files_are_rendered() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_compose_files_if_missing(dir.path(), "cov_pg", 5544).expect("compose files written");
    let compose = std::fs::read_to_string(dir.path().join("docker-compose.yaml")).unwrap();
    assert!(compose.contains("container_name: cov_pg"));
    assert!(compose.contains("\"5544:5432\""));
    let init =
        std::fs::read_to_string(dir.path().join("init-scripts").join("01-extensions.sql"))
            .unwrap();
    assert!(init.contains("uuid-ossp"));
}
