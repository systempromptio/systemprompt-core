#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli_integration_tests::full_bootstrap::{isolated_fixture, systemprompt_bin};
use systemprompt_test_fixtures::DisposableDb;

#[tokio::test]
async fn setup_authors_a_profile_and_migrates_its_owned_empty_database_via_reexec() {
    let database = DisposableDb::create("cli_setup_migrate")
        .await
        .expect("create empty isolated database");
    let url = url::Url::parse(database.url()).expect("fixture database URL");
    let host = url.host_str().expect("database host");
    let port = url.port_or_known_default().expect("database port");
    let user = url.username();
    let password = url.password().expect("database password");
    let name = url.path().trim_start_matches('/');
    let fixture = isolated_fixture(8080);
    let project = fixture
        .profile_path
        .parent()
        .and_then(std::path::Path::parent)
        .expect("fixture project root");
    std::fs::create_dir_all(project.join("target/release")).unwrap();
    std::fs::create_dir_all(project.join("web")).unwrap();

    let mut command = assert_cmd::Command::new(systemprompt_bin());
    command
        .current_dir(project)
        .env_remove("RUST_LOG")
        .env_remove("SYSTEMPROMPT_PROFILE")
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .args(["--non-interactive", "--no-color", "admin", "setup"])
        .args(["--environment", "coverage"])
        .args(["--db-host", host])
        .args(["--db-port", &port.to_string()])
        .args(["--db-user", user])
        .args(["--db-password", password])
        .args(["--db-name", name])
        .args(["--admin-email", "coverage-admin@test.invalid"])
        .args(["--openai-key", "synthetic-setup-provider-key"])
        .args(["--default-provider", "openai"])
        .args(["--migrate", "--yes"])
        .timeout(std::time::Duration::from_secs(120));
    let output = command.output().expect("run real setup binary");
    let sanitize = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .replace(database.url(), "postgres://***")
            .replace(password, "***")
    };
    assert!(
        output.status.success(),
        "setup failed\nstdout:\n{}\nstderr:\n{}",
        sanitize(&output.stdout),
        sanitize(&output.stderr)
    );
    let combined = format!("{}\n{}", sanitize(&output.stdout), sanitize(&output.stderr));
    assert!(
        combined.contains("Migrations completed successfully"),
        "{combined}"
    );

    let profile_path = project.join(".systemprompt/profiles/coverage/profile.yaml");
    let secrets_path = project.join(".systemprompt/profiles/coverage/secrets.json");
    assert!(profile_path.is_file(), "setup did not persist its profile");
    assert!(secrets_path.is_file(), "setup did not persist its secrets");
    let profile: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(&profile_path).expect("read authored profile"),
    )
    .expect("authored profile remains valid YAML");
    assert_eq!(profile["name"], "coverage");
    assert_eq!(
        profile["system_admin"]["email"],
        "coverage-admin@test.invalid"
    );
    let secrets: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&secrets_path).expect("read authored secrets"),
    )
    .expect("authored secrets remain valid JSON");
    assert!(
        secrets["database_url"].as_str() == Some(database.url()),
        "setup persisted an unexpected database URL"
    );
    assert!(
        secrets["openai"].as_str() == Some("synthetic-setup-provider-key"),
        "setup persisted an unexpected provider key"
    );

    let pool = database.pool().await.expect("migrated database pool");
    let raw = pool.pool_arc().expect("raw pool");
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM extension_migrations")
        .fetch_one(raw.as_ref())
        .await
        .expect("read migration ledger created by setup wrapper");
    assert!(
        applied > 10,
        "setup wrapper did not install extension schemas"
    );

    drop(raw);
    pool.write_pool_arc().unwrap().close().await;
    drop(pool);
    database.drop_now().await;
}
