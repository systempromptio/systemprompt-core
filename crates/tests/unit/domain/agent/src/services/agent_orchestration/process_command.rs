// Tests for agent subprocess command construction and log-file handling:
// log rotation thresholds, log-file creation, and the environment/argument
// shape of the spawn command (env_clear plus the sanctioned pass-through set).

use std::fs;

use systemprompt_agent::services::agent_orchestration::process::command::{
    BuildAgentCommandParams, build_agent_command, prepare_agent_log_file, rotate_log_if_needed,
};
use systemprompt_models::{CliPaths, Secrets};

fn secrets() -> Secrets {
    Secrets::parse(
        r#"{
            "oauth_at_rest_pepper": "0123456789abcdef0123456789abcdef",
            "database_url": "postgres://user:pass@localhost:5432/db",
            "anthropic": "sk-ant-test"
        }"#,
    )
    .expect("secrets parse")
}

#[test]
fn rotate_log_if_needed_ignores_small_and_missing_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("absent.log");
    rotate_log_if_needed(&missing).expect("missing file is a no-op");

    let small = dir.path().join("small.log");
    fs::write(&small, b"tiny").expect("write");
    rotate_log_if_needed(&small).expect("small file is a no-op");
    assert!(small.exists());
    assert!(!small.with_extension("log.old").exists());
}

#[test]
fn rotate_log_if_needed_rotates_oversized_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("big.log");
    let file = fs::File::create(&log).expect("create");
    file.set_len(11 * 1024 * 1024).expect("grow");
    drop(file);

    rotate_log_if_needed(&log).expect("rotation");
    assert!(!log.exists());
    assert!(log.with_extension("log.old").exists());
}

#[test]
fn prepare_agent_log_file_creates_directory_and_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_dir = dir.path().join("nested/logs");

    let file = prepare_agent_log_file("cmd_test", &log_dir).expect("log file");
    drop(file);
    assert!(log_dir.join("agent-cmd_test.log").exists());
}

#[test]
fn build_agent_command_sets_args_and_scoped_env() {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    let url = systemprompt_test_fixtures::fixture_database_url().expect("url");
    let config = systemprompt_test_fixtures::fixture_config(&url);

    let dir = tempfile::tempdir().expect("tempdir");
    let log_file = prepare_agent_log_file("cmd_env", dir.path()).expect("log file");

    let binary_path = bootstrap.bin_path.join("systemprompt");
    let secrets = secrets();
    let command = build_agent_command(BuildAgentCommandParams {
        binary_path: &binary_path,
        agent_name: "cmd_env",
        port: 39470,
        profile_path: "/tmp/profile.yaml",
        secrets: &secrets,
        config: &config,
        log_file,
    });

    let args: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    for expected in CliPaths::agent_run_args() {
        assert!(args.iter().any(|a| a == expected));
    }
    assert!(args.iter().any(|a| a == "--agent-name"));
    assert!(args.iter().any(|a| a == "cmd_env"));
    assert!(args.iter().any(|a| a == "39470"));

    let envs: Vec<(String, Option<String>)> = command
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    let env = |key: &str| {
        envs.iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.clone())
    };
    assert_eq!(env("AGENT_PORT").as_deref(), Some("39470"));
    assert_eq!(
        env("SYSTEMPROMPT_PROFILE").as_deref(),
        Some("/tmp/profile.yaml")
    );
    assert_eq!(env("DATABASE_TYPE").as_deref(), Some("postgres"));
    assert_eq!(
        env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV).as_deref(),
        Some("1")
    );
    assert_eq!(
        env(systemprompt_models::subprocess::AGENT_NAME_ENV).as_deref(),
        Some("cmd_env")
    );
}
