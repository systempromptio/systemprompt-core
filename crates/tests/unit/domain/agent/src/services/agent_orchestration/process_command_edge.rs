// Failure and opt-in-env branches of the agent spawn-command builder that the
// happy-path suite in `process_command` never reaches: an unusable log
// directory, and the two environment variables that are forwarded to the child
// only when the parent itself carries them.

use std::fs;
use std::path::PathBuf;

use systemprompt_agent::services::agent_orchestration::process::command::{
    BuildAgentCommandParams, build_agent_command, prepare_agent_log_file,
};
use systemprompt_models::Secrets;

fn secrets() -> Secrets {
    Secrets::parse(
        r#"{
            "oauth_at_rest_pepper": "0123456789abcdef0123456789abcdef",
            "database_url": "postgres://user:pass@localhost:5432/db"
        }"#,
    )
    .expect("secrets parse")
}

fn log_file_in(dir: &std::path::Path) -> fs::File {
    prepare_agent_log_file("envagent", dir).expect("log file")
}

#[test]
fn prepare_agent_log_file_fails_when_the_log_directory_path_is_a_file() {
    let tmp = tempfile::tempdir().expect("tmp");
    let blocker = tmp.path().join("logs");
    fs::write(&blocker, b"not a directory").expect("write blocker");

    let err = prepare_agent_log_file("blocked", &blocker)
        .expect_err("a regular file cannot hold a log file");
    assert!(
        err.to_string().contains("Failed to create log file"),
        "unexpected error: {err}"
    );
}

#[test]
fn prepare_agent_log_file_appends_to_an_existing_log_rather_than_truncating() {
    let tmp = tempfile::tempdir().expect("tmp");
    let path = tmp.path().join("agent-envagent.log");
    fs::write(&path, b"earlier run\n").expect("seed log");

    drop(log_file_in(tmp.path()));

    assert_eq!(
        fs::read_to_string(&path).expect("read log"),
        "earlier run\n",
        "reopening the log must preserve what a previous run wrote"
    );
}

#[test]
fn build_agent_command_forwards_the_trust_allowlist_when_the_parent_carries_one() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let Ok(url) = systemprompt_test_fixtures::fixture_database_url() else {
        return;
    };
    let config = systemprompt_test_fixtures::fixture_config(&url);
    let tmp = tempfile::tempdir().expect("tmp");
    let binary = PathBuf::from("/bin/true");
    let creds = secrets();

    // SAFETY: nextest gives each test its own process, so this mutation is
    // local to this test.
    unsafe {
        std::env::set_var(
            systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV,
            "sealed.internal",
        );
        std::env::set_var("FLY_APP_NAME", "test-fly-app");
    }

    let command = build_agent_command(BuildAgentCommandParams {
        binary_path: &binary,
        agent_name: "envagent",
        port: 9401,
        profile_path: "/tmp/profile.yaml",
        secrets: &creds,
        config: &config,
        log_file: log_file_in(tmp.path()),
    });

    let envs: Vec<(String, String)> = command
        .get_envs()
        .filter_map(|(k, v)| {
            Some((
                k.to_string_lossy().into_owned(),
                v?.to_string_lossy().into_owned(),
            ))
        })
        .collect();

    assert!(
        envs.contains(&(
            systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV.to_owned(),
            "sealed.internal".to_owned()
        )),
        "the SSRF allowlist must travel to the child: {envs:?}"
    );
    assert!(
        envs.contains(&("FLY_APP_NAME".to_owned(), "test-fly-app".to_owned())),
        "the Fly app name must travel to the child: {envs:?}"
    );
}

#[test]
fn build_agent_command_omits_the_optional_env_vars_when_the_parent_lacks_them() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let Ok(url) = systemprompt_test_fixtures::fixture_database_url() else {
        return;
    };
    let config = systemprompt_test_fixtures::fixture_config(&url);
    let tmp = tempfile::tempdir().expect("tmp");
    let binary = PathBuf::from("/bin/true");
    let creds = secrets();

    // SAFETY: nextest gives each test its own process.
    unsafe {
        std::env::remove_var(systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV);
        std::env::remove_var("FLY_APP_NAME");
    }

    let command = build_agent_command(BuildAgentCommandParams {
        binary_path: &binary,
        agent_name: "envagent",
        port: 9401,
        profile_path: "/tmp/profile.yaml",
        secrets: &creds,
        config: &config,
        log_file: log_file_in(tmp.path()),
    });

    let names: Vec<String> = command
        .get_envs()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();

    assert!(
        !names.contains(&systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV.to_owned()),
        "an absent allowlist must not be forwarded as an empty one: {names:?}"
    );
    assert!(!names.contains(&"FLY_APP_NAME".to_owned()));
    assert!(
        names.contains(&"AGENT_PORT".to_owned()),
        "the mandatory child env is still set: {names:?}"
    );
}
