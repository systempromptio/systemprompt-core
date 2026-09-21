use std::time::Duration;

use assert_cmd::Command;
use systemprompt_cli_integration_tests::full_bootstrap::{
    TEST_MANIFEST_SIGNING_SEED, TEST_OAUTH_AT_REST_PEPPER, isolated_fixture, systemprompt_bin,
};
use systemprompt_test_fixtures::DisposableDb;

fn doctor(profile: &std::path::Path, database_url: &str) -> std::process::Output {
    let mut command = Command::new(systemprompt_bin());
    command.env_remove("RUST_LOG");
    command.env_remove("SYSTEMPROMPT_PROFILE");
    command.env("DATABASE_URL", database_url);
    command.env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
    command.env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
    command.env("SYSTEMPROMPT_SUBPROCESS", "1");
    let profile_dir = profile.parent().expect("profile directory");
    let project_root = profile_dir.parent().expect("owned project root");
    command.current_dir(project_root);
    command.args([
        "--non-interactive",
        "--no-color",
        "cloud",
        "doctor",
        "--profile",
    ]);
    command.arg(profile_dir);
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
    let database = DisposableDb::installed("cli_cloud_doctor_recovery")
        .await
        .expect("install isolated doctor database");
    let fixture = isolated_fixture(8080);
    let profile_dir = fixture.profile_path.parent().expect("profile directory");
    let secrets_path = profile_dir.join("secrets.json");
    assert!(
        !secrets_path.exists(),
        "isolated profile starts without secrets"
    );

    let missing = doctor(&fixture.profile_path, database.url());
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
            "anthropic": "synthetic-doctor-provider-secret",
            "openai": "synthetic-doctor-openai-secret",
            "signing_key_pem": signing_key
        }))
        .expect("serialize repaired secrets"),
    )
    .expect("write repaired secrets");

    let repaired = doctor(&fixture.profile_path, database.url());
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
