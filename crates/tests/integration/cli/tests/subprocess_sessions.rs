//! Subprocess tests for the CLI session resolution matrix: `--profile`
//! override, `SYSTEMPROMPT_PROFILE` env resolution, the stored active-key
//! path, cached-session revalidation on a second invocation, and the tenant
//! path that fails closed without cloud credentials.
//!
//! Each test isolates `HOME` and the working directory in a tempdir so
//! session stores never leak between tests or into the developer machine.

use std::path::{Path, PathBuf};

use predicates::prelude::*;
use systemprompt_cli_integration_tests::full_bootstrap::{
    cli_command, cli_command_bare, full_fixture,
};

fn isolated_home() -> tempfile::TempDir {
    full_fixture();
    tempfile::tempdir().expect("create isolated home")
}

fn session_probe_args() -> [&'static str; 4] {
    ["plugins", "mcp", "call", "no_such_server_for_session"]
}

fn find_session_store(home: &Path) -> Option<PathBuf> {
    let mut stack = vec![home.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == "index.json") {
                return Some(path);
            }
        }
    }
    None
}

fn store_json(home: &Path) -> serde_json::Value {
    let path = find_session_store(home).expect("session store written under HOME");
    let raw = std::fs::read_to_string(path).expect("read session store");
    serde_json::from_str(&raw).expect("parse session store")
}

#[test]
fn session_from_profile_flag_records_session_without_activating_it() {
    let home = isolated_home();
    let mut cmd = cli_command();
    cmd.env("HOME", home.path());
    cmd.current_dir(home.path());
    cmd.args(session_probe_args());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));

    let store = store_json(home.path());
    assert!(
        store["active_key"].is_null(),
        "an explicit --profile is a one-shot target and must not become the active session"
    );
    assert_eq!(
        store["sessions"].as_object().map(serde_json::Map::len),
        Some(1)
    );
}

#[test]
fn session_from_env_profile_resolves() {
    let home = isolated_home();
    let profile_path = full_fixture().profile_path.clone();
    let mut cmd = cli_command_bare();
    cmd.env("HOME", home.path());
    cmd.env("SYSTEMPROMPT_PROFILE", &profile_path);
    cmd.current_dir(home.path());
    cmd.args(session_probe_args());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"))
        .stderr(predicate::str::contains("Profile required").not());

    assert!(find_session_store(home.path()).is_some());
}

#[test]
fn profile_flag_alone_leaves_a_bare_invocation_unresolved() {
    let home = isolated_home();

    let mut first = cli_command();
    first.env("HOME", home.path());
    first.current_dir(home.path());
    first.args(session_probe_args());
    first.assert().failure();

    let mut second = cli_command_bare();
    second.env("HOME", home.path());
    second.current_dir(home.path());
    second.args(session_probe_args());
    second
        .assert()
        .failure()
        .stderr(predicate::str::contains("Profile resolution failed"));
}

#[test]
fn session_from_stored_active_key_resolves_after_switch() {
    let home = isolated_home();
    let fixture_profile = full_fixture().profile_path.clone();
    let profile_dir = home.path().join(".systemprompt/profiles/switched");
    std::fs::create_dir_all(&profile_dir).expect("mkdir profiles dir");
    std::fs::copy(&fixture_profile, profile_dir.join("profile.yaml"))
        .expect("copy fixture profile");

    let mut switch = cli_command();
    switch.env("HOME", home.path());
    switch.current_dir(home.path());
    switch.args(["admin", "session", "switch", "switched"]);
    switch.assert().success();

    let store = store_json(home.path());
    assert_eq!(store["active_profile_name"], "switched");
    assert!(store["active_key"].is_string());

    let mut login = cli_command_bare();
    login.env("HOME", home.path());
    login.current_dir(home.path());
    login.args(["admin", "session", "login"]);
    login.assert().success();

    let mut bare = cli_command_bare();
    bare.env("HOME", home.path());
    bare.current_dir(home.path());
    bare.args(session_probe_args());
    bare.assert()
        .failure()
        .stderr(predicate::str::contains("not found"))
        .stderr(predicate::str::contains("Profile resolution failed").not());
}

#[test]
fn cached_session_is_reused_on_second_invocation() {
    let home = isolated_home();

    for _ in 0..2 {
        let mut cmd = cli_command();
        cmd.env("HOME", home.path());
        cmd.current_dir(home.path());
        cmd.args(session_probe_args());
        cmd.assert().failure();
    }

    let store = store_json(home.path());
    let sessions = store["sessions"].as_object().expect("sessions map");
    assert_eq!(sessions.len(), 1);
    let session = sessions.values().next().expect("one session");
    assert!(!session["session_token"].as_str().unwrap_or("").is_empty());
}

#[test]
fn no_profile_anywhere_reports_profile_required() {
    let home = isolated_home();
    let mut cmd = cli_command_bare();
    cmd.env("HOME", home.path());
    cmd.current_dir(home.path());
    cmd.args(session_probe_args());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Profile resolution failed"));
}

#[test]
fn tenant_profile_without_credentials_fails_closed() {
    let home = isolated_home();
    let profile_path = full_fixture().profile_path.clone();
    let base = std::fs::read_to_string(&profile_path).expect("read fixture profile");

    let tenant_dir = home.path().join("tenantprof");
    std::fs::create_dir_all(&tenant_dir).expect("mkdir tenant profile dir");
    let tenant_profile = tenant_dir.join("profile.yaml");
    std::fs::write(
        &tenant_profile,
        format!("{base}cloud:\n  tenant_id: tn_cov_fixture\n"),
    )
    .expect("write tenant profile");

    let mut cmd = cli_command_bare();
    cmd.env("HOME", home.path());
    cmd.current_dir(home.path());
    cmd.arg("--profile").arg(&tenant_profile);
    cmd.args(session_probe_args());
    cmd.assert().failure().stderr(
        predicate::str::contains("Cloud authentication")
            .or(predicate::str::contains("credentials")),
    );
}
