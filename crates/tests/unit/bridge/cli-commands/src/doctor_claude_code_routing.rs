//! The doctor's "Claude Code asks for a login" diagnosis: routing judged over
//! the files Claude Code reads, a relocated config dir, and environment
//! credentials that outrank the helper.

use std::path::{Path, PathBuf};

use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::claude_code::{
    check_config_dir_override, check_effective_routing, check_env_credentials, read_paths,
};
use systemprompt_bridge::integration::claude_code_routing::routes_through_gateway;
use tempfile::TempDir;

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write settings");
    path
}

#[test]
fn routing_fails_when_no_read_file_carries_base_url_and_helper() {
    let dir = TempDir::new().expect("dir");
    let user = write(
        dir.path(),
        "settings.json",
        r#"{"model": "claude-sonnet-5"}"#,
    );
    let missing = dir.path().join("managed-settings.json");

    let check = check_effective_routing(&[missing, user]);

    assert_eq!(check.status, Status::Fail);
    assert!(check.detail.contains("ask for a login"), "{}", check.detail);
    assert!(
        check.detail.contains("install --host claude-code"),
        "{}",
        check.detail
    );
    assert!(check.detail.contains("--settings"), "{}", check.detail);
}

#[test]
fn routing_fails_when_only_the_base_url_is_present() {
    let dir = TempDir::new().expect("dir");
    let user = write(
        dir.path(),
        "settings.json",
        r#"{"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:48217"}}"#,
    );

    assert_eq!(check_effective_routing(&[user]).status, Status::Fail);
}

#[test]
fn routing_passes_when_policy_and_user_files_together_route() {
    let dir = TempDir::new().expect("dir");
    let policy = write(
        dir.path(),
        "managed-settings.json",
        r#"{"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:48217"}}"#,
    );
    let user = write(
        dir.path(),
        "settings.json",
        r#"{"apiKeyHelper": "/bin/helper"}"#,
    );

    let check = check_effective_routing(&[policy, user]);

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(
        !check.detail.contains("/bin/helper"),
        "the helper command is never echoed"
    );
}

#[test]
fn claude_config_dir_redirects_the_user_settings_file() {
    let dir = TempDir::new().expect("dir");
    let paths = read_paths(Some(dir.path()));
    assert_eq!(paths.last(), Some(&dir.path().join("settings.json")));
    assert!(paths[0].ends_with("managed-settings.json"));
}

#[test]
fn claude_config_dir_is_warned_about() {
    let check = check_config_dir_override(Path::new("/tmp/elsewhere"));
    assert_eq!(check.status, Status::Warn);
    assert!(
        check.detail.contains("CLAUDE_CONFIG_DIR"),
        "{}",
        check.detail
    );
}

#[test]
fn environment_credentials_that_outrank_the_helper_are_warned_about() {
    assert!(check_env_credentials(|_| false).is_none());
    let check = check_env_credentials(|key| key == "ANTHROPIC_AUTH_TOKEN").expect("warned");
    assert_eq!(check.status, Status::Warn);
    assert!(
        check.detail.contains("ANTHROPIC_AUTH_TOKEN"),
        "{}",
        check.detail
    );
    assert!(
        !check.detail.contains("ANTHROPIC_API_KEY"),
        "{}",
        check.detail
    );
}

// The GUI's Claude Code verdict reads `routes_through_gateway`; the doctor's
// check must pass exactly when it does, or the two disagree about one machine.
#[test]
fn the_doctor_check_and_the_gui_verdict_agree_on_routing() {
    let dir = TempDir::new().expect("dir");
    for (body, routed) in [
        (r#"{"model": "claude-sonnet-5"}"#, false),
        (r#"{"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:48217"}}"#, false),
        (r#"{"apiKeyHelper": "/bin/helper"}"#, false),
        (
            r#"{"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:48217"}, "apiKeyHelper": "/bin/h"}"#,
            true,
        ),
        ("{ not json", false),
    ] {
        let user = write(dir.path(), "settings.json", body);
        let paths = [dir.path().join("managed-settings.json"), user];
        assert_eq!(routes_through_gateway(&paths), routed, "{body}");
        let status = check_effective_routing(&paths).status;
        assert_eq!(status == Status::Ok, routed, "{body}: {status:?}");
    }
}
