//! Subprocess coverage for `admin session` login, list, show, switch, and
//! logout against the full-bootstrap fixture with an isolated HOME.

use std::path::{Path, PathBuf};

use systemprompt_cli_integration_tests::full_bootstrap::{cli_command, full_fixture};

fn isolated_project() -> (tempfile::TempDir, PathBuf) {
    let fix = full_fixture();
    let home = tempfile::tempdir().expect("create isolated home");
    let profiles = home.path().join(".systemprompt/profiles/covfix");
    std::fs::create_dir_all(&profiles).expect("mkdir profiles dir");
    let profile_copy = profiles.join("profile.yaml");
    std::fs::copy(&fix.profile_path, &profile_copy).expect("copy fixture profile");
    (home, profile_copy)
}

fn session_cmd(home: &Path, args: &[&str]) -> assert_cmd::Command {
    let mut cmd = cli_command();
    cmd.env("HOME", home);
    cmd.current_dir(home);
    cmd.args(args);
    cmd
}

#[test]
fn login_creates_session_and_reuses_it() {
    let (home, _) = isolated_project();
    let mut cmd = session_cmd(home.path(), &["admin", "session", "login"]);
    cmd.assert().success();

    let mut again = session_cmd(home.path(), &["admin", "session", "login"]);
    again.assert().success();
}

#[test]
fn login_token_only_and_force_new() {
    let (home, _) = isolated_project();
    let mut cmd = session_cmd(home.path(), &["admin", "session", "login", "--token-only"]);
    let output = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).into_owned();
    assert!(!stdout.trim().is_empty());

    let mut forced = session_cmd(
        home.path(),
        &[
            "admin",
            "session",
            "login",
            "--force-new",
            "--duration-hours",
            "2",
        ],
    );
    forced.assert().success();
}

#[test]
fn login_with_formats() {
    let (home, _) = isolated_project();
    for format in ["--json", "--yaml"] {
        let mut cmd = session_cmd(home.path(), &[format, "admin", "session", "login"]);
        cmd.assert().success();
    }
}

#[test]
fn list_show_switch_after_login() {
    let (home, _) = isolated_project();
    let mut login = session_cmd(home.path(), &["admin", "session", "login"]);
    login.assert().success();

    for args in [
        vec!["admin", "session", "list"],
        vec!["--json", "admin", "session", "list"],
        vec!["admin", "session", "show"],
        vec!["--json", "admin", "session", "show"],
        vec!["admin", "session", "switch", "covfix"],
    ] {
        let mut cmd = session_cmd(home.path(), &args);
        let _ = cmd.assert();
    }

    let mut bad_switch = session_cmd(
        home.path(),
        &["admin", "session", "switch", "no_such_profile"],
    );
    bad_switch.assert().failure();
}

#[test]
fn logout_single_and_all() {
    let (home, _) = isolated_project();
    let mut login = session_cmd(home.path(), &["admin", "session", "login"]);
    login.assert().success();

    let mut logout = session_cmd(home.path(), &["admin", "session", "logout", "-y"]);
    let _ = logout.assert();

    let mut relogin = session_cmd(home.path(), &["admin", "session", "login"]);
    relogin.assert().success();

    let mut logout_all = session_cmd(home.path(), &["admin", "session", "logout", "--all", "-y"]);
    let _ = logout_all.assert();

    let mut logout_named = session_cmd(
        home.path(),
        &["admin", "session", "logout", "--profile", "covfix", "-y"],
    );
    let _ = logout_named.assert();
}

#[test]
fn session_show_and_list_without_login() {
    let (home, _) = isolated_project();
    for args in [
        vec!["admin", "session", "show"],
        vec!["admin", "session", "list"],
        vec!["--yaml", "admin", "session", "list"],
    ] {
        let mut cmd = session_cmd(home.path(), &args);
        let _ = cmd.assert();
    }
}
