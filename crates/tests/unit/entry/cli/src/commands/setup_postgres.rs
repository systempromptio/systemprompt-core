//! Tests for the setup wizard's PostgreSQL provisioning flows, driven with
//! `ScriptedPrompter` and a scratch database resolved from `DATABASE_URL`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::setup::SetupArgs;
use systemprompt_cli::admin::setup::docker_compose::create_compose_files_if_missing;
use systemprompt_cli::admin::setup::postgres::{setup_interactive, setup_non_interactive};
use systemprompt_cli::admin::setup::postgres_access::{
    PostgresConfig, detect_postgresql, enable_extensions, generate_password, test_connection,
};
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
        port_offset: 0,
        db_user: None,
        db_password: None,
        db_password_stdin: false,
        db_name: None,
        gemini_key: None,
        anthropic_key: None,
        openai_key: None,
        github_token: None,
        default_provider: None,
        admin_email: None,
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

fn db_url() -> DbUrl {
    let raw = systemprompt_test_fixtures::test_database_url();
    let url = url::Url::parse(&raw).expect("DATABASE_URL parses as a URL");
    DbUrl {
        user: url.username().to_owned(),
        password: url
            .password()
            .map(str::to_owned)
            .or_else(|| std::env::var("PGPASSWORD").ok())
            .expect("DATABASE_URL or PGPASSWORD carries a password"),
        host: url
            .host_str()
            .expect("DATABASE_URL carries a host")
            .to_owned(),
        port: url.port().unwrap_or(5432),
        database: url.path().trim_start_matches('/').to_owned(),
    }
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

// Why: the assertion above describes the password's shape, and a constant
// sixteen-character string satisfies it. This is the database password every
// installation is set up with, so a generator that stopped varying would give
// every install the same one — and the shape check would not notice.
#[test]
fn two_generated_passwords_differ() {
    let first = generate_password();
    let second = generate_password();

    assert_ne!(
        first, second,
        "a password generator that repeats itself is a shared credential"
    );
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
    assert_eq!(
        config.database_url().unwrap(),
        "postgres://u:p@localhost:5433/d"
    );
}

#[test]
fn database_url_percent_encodes_credentials_with_url_delimiters() {
    let config = PostgresConfig {
        host: "db.internal".to_owned(),
        port: 5433,
        user: "ops@team".to_owned(),
        password: "p@ss/w#rd:?".to_owned(),
        database: "d".to_owned(),
    };
    let rendered = config.database_url().unwrap();
    let parsed = url::Url::parse(&rendered).unwrap();
    assert_eq!(parsed.host_str(), Some("db.internal"));
    assert_eq!(parsed.port(), Some(5433));
    assert_eq!(parsed.path(), "/d");
    assert_eq!(parsed.username(), "ops%40team");
    assert_eq!(parsed.password(), Some("p%40ss%2Fw%23rd%3A%3F"));
}

#[test]
fn enable_extensions_fails_when_it_cannot_connect() {
    let config = PostgresConfig {
        host: "127.0.0.1".to_owned(),
        port: 9,
        user: "nobody".to_owned(),
        password: "nothing".to_owned(),
        database: "nowhere".to_owned(),
    };
    let error = runtime()
        .block_on(enable_extensions(&config))
        .expect_err("an unreachable database cannot have its extensions enabled");
    assert!(error.to_string().contains("enable extensions"), "{error:#}");
}

fn scratch_name(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn admin_pool(db: &DbUrl) -> sqlx::PgPool {
    let options = sqlx::postgres::PgConnectOptions::new()
        .host(&db.host)
        .port(db.port)
        .username(&db.user)
        .password(&db.password)
        .database("postgres");
    sqlx::PgPool::connect_with(options).await.expect("connect")
}

async fn drop_scratch(db: &DbUrl, role: &str, database: &str) {
    let pool = admin_pool(db).await;
    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS \"{database}\" WITH (FORCE)"
    )))
    .execute(&pool)
    .await;
    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP ROLE IF EXISTS \"{role}\""
    )))
    .execute(&pool)
    .await;
    pool.close().await;
}

#[test]
fn rerun_against_an_existing_role_sets_the_password_the_profile_carries() {
    let db = db_url();
    let role = scratch_name("c1_setup_role");
    let database = scratch_name("c1_setup_db");
    runtime().block_on(async {
        let pool = admin_pool(&db).await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE ROLE \"{role}\" LOGIN PASSWORD 'the_old_password'"
        )))
        .execute(&pool)
        .await
        .expect("create the pre-existing role");
        pool.close().await;
    });

    let mut setup_args = args();
    setup_args.db_password = Some("new p@ss/w#rd".to_owned());
    let port = db.port.to_string();
    let prompter = scripted(&[
        "0",
        db.host.as_str(),
        &port,
        &role,
        &database,
        "y",
        db.user.as_str(),
        db.password.as_str(),
    ]);
    let outcome = runtime().block_on(setup_interactive(
        &setup_args,
        &prompter,
        "covtest",
        &CliConfig::default(),
    ));
    let connects = outcome
        .as_ref()
        .is_ok_and(|config| runtime().block_on(test_connection(config)));
    runtime().block_on(drop_scratch(&db, &role, &database));

    let config = outcome.expect("setup against an existing role succeeds");
    assert_eq!(config.password, "new p@ss/w#rd");
    assert!(
        connects,
        "the profile's password must log in as the pre-existing role"
    );
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
    let db = db_url();
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
    let db = db_url();
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
    let db = db_url();
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
    let db = db_url();
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
    let db = db_url();
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
    let db = db_url();
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
fn docker_non_interactive_reuses_reachable_database_without_compose() {
    let db = db_url();
    let dir = systemprompt_test_fixtures::canonical_tempdir();
    std::env::set_current_dir(dir.path()).expect("chdir");

    let config = PostgresConfig {
        host: db.host,
        port: db.port,
        user: db.user,
        password: db.password,
        database: db.database,
    };
    let result = runtime()
        .block_on(
            systemprompt_cli::admin::setup::docker::setup_docker_postgres_non_interactive(
                &config, "covtest",
            ),
        )
        .expect("reachable database is reused as-is");
    assert_eq!(result.database, config.database);
    assert!(!dir.path().join("infrastructure/docker").exists());
}

#[test]
fn docker_non_interactive_refuses_occupied_port() {
    use systemprompt_cli::admin::setup::docker_compose::{
        is_compose_available, is_docker_available,
    };
    if !is_docker_available() || !is_compose_available() {
        // skip-ok: Docker and docker compose are not installed on this host.
        return;
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let dir = systemprompt_test_fixtures::canonical_tempdir();
    std::env::set_current_dir(dir.path()).expect("chdir");

    let config = PostgresConfig {
        host: "127.0.0.1".to_owned(),
        port,
        user: "cov_wrong_user".to_owned(),
        password: "cov_wrong_pw".to_owned(),
        database: "cov_wrong_db".to_owned(),
    };
    let err = runtime()
        .block_on(
            systemprompt_cli::admin::setup::docker::setup_docker_postgres_non_interactive(
                &config,
                "covtestport",
            ),
        )
        .unwrap_err();
    assert!(err.to_string().contains("already in use"));
    assert!(!dir.path().join("infrastructure/docker").exists());
}

#[test]
fn compose_files_are_rendered() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_compose_files_if_missing(dir.path(), "cov_pg", 5544).expect("compose files written");
    let compose = std::fs::read_to_string(dir.path().join("docker-compose.yaml")).unwrap();
    assert!(compose.contains("container_name: cov_pg"));
    assert!(compose.contains("\"5544:5432\""));
    let init =
        std::fs::read_to_string(dir.path().join("init-scripts").join("01-extensions.sql")).unwrap();
    assert!(init.contains("uuid-ossp"));
}

#[test]
fn docker_probes_do_not_panic() {
    use systemprompt_cli::admin::setup::docker_compose::{
        is_compose_available, is_container_running, is_docker_available,
    };
    let _ = is_docker_available();
    let _ = is_compose_available();
    assert!(!is_container_running("cov_no_such_container_zzz"));
}
