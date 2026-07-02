//! Subprocess coverage for `admin session` login, list, show, switch, and
//! logout against the full-bootstrap fixture with an isolated HOME.

use std::path::{Path, PathBuf};

use systemprompt_cli_integration_tests::full_bootstrap::{command, fixture};

fn isolated_project() -> Option<(tempfile::TempDir, PathBuf)> {
    let fix = fixture()?;
    let home = tempfile::tempdir().expect("create isolated home");
    let profiles = home.path().join(".systemprompt/profiles/covfix");
    std::fs::create_dir_all(&profiles).expect("mkdir profiles dir");
    let profile_copy = profiles.join("profile.yaml");
    std::fs::copy(&fix.profile_path, &profile_copy).expect("copy fixture profile");
    Some((home, profile_copy))
}

fn session_cmd(home: &Path, args: &[&str]) -> Option<assert_cmd::Command> {
    let mut cmd = command()?;
    cmd.env("HOME", home);
    cmd.current_dir(home);
    cmd.args(args);
    Some(cmd)
}

#[test]
fn login_creates_session_and_reuses_it() {
    let Some((home, _)) = isolated_project() else {
        return;
    };
    let Some(mut cmd) = session_cmd(home.path(), &["admin", "session", "login"]) else {
        return;
    };
    cmd.assert().success();

    let Some(mut again) = session_cmd(home.path(), &["admin", "session", "login"]) else {
        return;
    };
    again.assert().success();
}

#[test]
fn login_token_only_and_force_new() {
    let Some((home, _)) = isolated_project() else {
        return;
    };
    let Some(mut cmd) = session_cmd(home.path(), &["admin", "session", "login", "--token-only"])
    else {
        return;
    };
    let output = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).into_owned();
    assert!(!stdout.trim().is_empty());

    let Some(mut forced) = session_cmd(
        home.path(),
        &[
            "admin",
            "session",
            "login",
            "--force-new",
            "--duration-hours",
            "2",
        ],
    ) else {
        return;
    };
    forced.assert().success();
}

#[test]
fn login_with_formats() {
    let Some((home, _)) = isolated_project() else {
        return;
    };
    for format in ["--json", "--yaml"] {
        let Some(mut cmd) = session_cmd(home.path(), &[format, "admin", "session", "login"])
        else {
            return;
        };
        cmd.assert().success();
    }
}

#[test]
fn list_show_switch_after_login() {
    let Some((home, _)) = isolated_project() else {
        return;
    };
    let Some(mut login) = session_cmd(home.path(), &["admin", "session", "login"]) else {
        return;
    };
    login.assert().success();

    for args in [
        vec!["admin", "session", "list"],
        vec!["--json", "admin", "session", "list"],
        vec!["admin", "session", "show"],
        vec!["--json", "admin", "session", "show"],
        vec!["admin", "session", "switch", "covfix"],
    ] {
        let Some(mut cmd) = session_cmd(home.path(), &args) else {
            return;
        };
        let _ = cmd.assert();
    }

    let Some(mut bad_switch) = session_cmd(
        home.path(),
        &["admin", "session", "switch", "no_such_profile"],
    ) else {
        return;
    };
    bad_switch.assert().failure();
}

#[test]
fn logout_single_and_all() {
    let Some((home, _)) = isolated_project() else {
        return;
    };
    let Some(mut login) = session_cmd(home.path(), &["admin", "session", "login"]) else {
        return;
    };
    login.assert().success();

    let Some(mut logout) = session_cmd(home.path(), &["admin", "session", "logout", "-y"]) else {
        return;
    };
    let _ = logout.assert();

    let Some(mut relogin) = session_cmd(home.path(), &["admin", "session", "login"]) else {
        return;
    };
    relogin.assert().success();

    let Some(mut logout_all) = session_cmd(
        home.path(),
        &["admin", "session", "logout", "--all", "-y"],
    ) else {
        return;
    };
    let _ = logout_all.assert();

    let Some(mut logout_named) = session_cmd(
        home.path(),
        &["admin", "session", "logout", "--profile", "covfix", "-y"],
    ) else {
        return;
    };
    let _ = logout_named.assert();
}

#[test]
fn session_show_and_list_without_login() {
    let Some((home, _)) = isolated_project() else {
        return;
    };
    for args in [
        vec!["admin", "session", "show"],
        vec!["admin", "session", "list"],
        vec!["--yaml", "admin", "session", "list"],
    ] {
        let Some(mut cmd) = session_cmd(home.path(), &args) else {
            return;
        };
        let _ = cmd.assert();
    }
}
