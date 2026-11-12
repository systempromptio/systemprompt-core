use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;
use systemprompt_cli_integration_tests::full_bootstrap::{
    TEST_ENCRYPTION_MASTER_KEY, TEST_MANIFEST_SIGNING_SEED, TEST_OAUTH_AT_REST_PEPPER,
    isolated_fixture, systemprompt_bin,
};
use systemprompt_test_fixtures::DisposableDb;

const PROFILE_NAME: &str = "doctor_recovery";

fn place_in_project(fixture_profile: &Path) -> (PathBuf, PathBuf) {
    let project_root = fixture_profile
        .parent()
        .and_then(Path::parent)
        .expect("owned project root")
        .to_path_buf();
    let profile_dir = project_root
        .join(".systemprompt/profiles")
        .join(PROFILE_NAME);
    std::fs::create_dir_all(&profile_dir).expect("create discoverable profile directory");
    std::fs::copy(fixture_profile, profile_dir.join("profile.yaml"))
        .expect("place profile where name discovery finds it");
    (project_root, profile_dir)
}

fn doctor(project_root: &Path, database_url: &str) -> std::process::Output {
    let mut command = Command::new(systemprompt_bin());
    command.env_remove("RUST_LOG");
    command.env_remove("SYSTEMPROMPT_PROFILE");
    command.env("DATABASE_URL", database_url);
    command.env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
    command.env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
    command.env("SYSTEMPROMPT_CUSTOM_SECRETS", "encryption_master_key");
    command.env("encryption_master_key", TEST_ENCRYPTION_MASTER_KEY);
    command.env("SYSTEMPROMPT_SUBPROCESS", "1");
    command.current_dir(project_root);
    command.args([
        "--non-interactive",
        "--no-color",
        "cloud",
        "doctor",
        "--profile",
        PROFILE_NAME,
    ]);
    command.timeout(Duration::from_secs(120));
    command.output().expect("run bounded cloud doctor")
}

fn redact(output: &[u8], database_url: &str) -> String {
    let mut sanitized = String::from_utf8_lossy(output).replace(database_url, "<database-url>");
    if let Ok(parsed) = url::Url::parse(database_url)
        && let Some(password) = parsed.password().filter(|value| !value.is_empty())
    {
        sanitized = sanitized.replace(password, "<database-password>");
    }
    sanitized
}

#[tokio::test]
async fn cloud_doctor_reports_missing_secrets_then_accepts_repaired_owned_profile() {
    let database = DisposableDb::with_schema("cli_cloud_doctor_recovery").await;
    let fixture = isolated_fixture(8080);
    let (project_root, profile_dir) = place_in_project(&fixture.profile_path);
    let secrets_path = profile_dir.join("secrets.json");
    assert!(
        !secrets_path.exists(),
        "isolated profile starts without secrets"
    );

    let missing = doctor(&project_root, database.url());
    assert!(
        !missing.status.success(),
        "missing deployment secrets must block preflight"
    );
    let missing_stdout = redact(&missing.stdout, database.url());
    let missing_stderr = redact(&missing.stderr, database.url());
    let missing_output = format!("{missing_stdout}\n{missing_stderr}");
    assert!(missing_output.contains("secrets-file"), "{missing_output}");
    assert!(
        missing_output.contains("not found or unreadable"),
        "{missing_output}"
    );
    assert!(
        missing_output.contains("Deploy preflight failed"),
        "{missing_output}"
    );

    let signing_key = std::fs::read_to_string(fixture.system_dir.join("signing_key.pem"))
        .expect("read owned signing key");
    std::fs::write(
        &secrets_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "database_url": database.url(),
            "oauth_at_rest_pepper": TEST_OAUTH_AT_REST_PEPPER,
            "encryption_master_key": TEST_ENCRYPTION_MASTER_KEY,
            "anthropic": "synthetic-doctor-provider-secret",
            "openai": "synthetic-doctor-openai-secret",
            "signing_key_pem": signing_key
        }))
        .expect("serialize repaired secrets"),
    )
    .expect("write repaired secrets");

    let repaired = doctor(&project_root, database.url());
    let raw_repaired_stdout = String::from_utf8_lossy(&repaired.stdout);
    let raw_repaired_stderr = String::from_utf8_lossy(&repaired.stderr);
    let database_password = url::Url::parse(database.url())
        .ok()
        .and_then(|url| url.password().map(str::to_owned));
    for secret in [
        database.url(),
        database_password.as_deref().unwrap_or(""),
        "synthetic-doctor-provider-secret",
        "synthetic-doctor-openai-secret",
    ] {
        if !secret.is_empty() {
            assert!(
                !raw_repaired_stdout.contains(secret),
                "stdout exposed a configured secret"
            );
            assert!(
                !raw_repaired_stderr.contains(secret),
                "stderr exposed a configured secret"
            );
        }
    }
    let repaired_stdout = redact(&repaired.stdout, database.url());
    let repaired_stderr = redact(&repaired.stderr, database.url());
    assert!(
        repaired.status.success(),
        "repaired profile must pass: stdout={repaired_stdout} stderr={repaired_stderr}"
    );
    let repaired_output = format!("{repaired_stdout}\n{repaired_stderr}");
    assert!(
        !repaired_output.contains("secrets-file"),
        "a readable secrets file must not emit the failure-only check: {repaired_output}"
    );
    assert!(
        repaired_output.contains("required keys present"),
        "{repaired_output}"
    );
    assert!(
        repaired_output.contains("all provider credentials present"),
        "{repaired_output}"
    );
    assert!(
        repaired_output.contains("Deploy preflight passed"),
        "{repaired_output}"
    );

    database.drop_now().await;
}
