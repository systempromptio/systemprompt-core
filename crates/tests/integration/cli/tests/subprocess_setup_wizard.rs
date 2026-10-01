//! Subprocess coverage for `admin setup`: dry-run preview and the full
//! non-interactive profile/secrets generation flow in an isolated project.

use std::path::Path;

use predicates::prelude::*;
use systemprompt_cli_integration_tests::full_bootstrap::{cli_command_bare, full_fixture};
use systemprompt_test_fixtures::test_database_url;

struct DbParts {
    host: String,
    port: String,
    user: String,
    password: String,
}

fn db_parts() -> DbParts {
    let raw = test_database_url();
    let url = url::Url::parse(&raw).expect("DATABASE_URL parses as a URL");
    DbParts {
        host: url
            .host_str()
            .expect("DATABASE_URL carries a host")
            .to_owned(),
        port: url.port().unwrap_or(5432).to_string(),
        user: url.username().to_owned(),
        password: url
            .password()
            .expect("DATABASE_URL carries a password")
            .to_owned(),
    }
}

fn project_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create project dir");
    std::fs::create_dir_all(dir.path().join("services")).expect("mkdir services");
    dir
}

fn setup_cmd(project: &Path, args: &[&str]) -> assert_cmd::Command {
    let mut cmd = cli_command_bare();
    cmd.env("HOME", project);
    cmd.current_dir(project);
    cmd.args(args);
    cmd
}

#[test]
fn setup_dry_run_previews_without_writing() {
    full_fixture();
    let project = project_dir();
    let mut cmd = setup_cmd(
        project.path(),
        &[
            "admin",
            "setup",
            "--dry-run",
            "-y",
            "--environment",
            "covsetup",
            "--db-host",
            "127.0.0.1",
            "--db-port",
            "9",
            "--anthropic-key",
            "sk-cov-test",
            "--no-migrate",
        ],
    );
    cmd.assert().success();
    assert!(
        !project
            .path()
            .join(".systemprompt/profiles/covsetup/profile.yaml")
            .exists()
    );
}

#[test]
fn setup_full_non_interactive_writes_profile_and_secrets() {
    let db = db_parts();
    full_fixture();
    let project = project_dir();
    let base = [
        "admin",
        "setup",
        "-y",
        "--environment",
        "covsetup",
        "--db-host",
        db.host.as_str(),
        "--db-port",
        db.port.as_str(),
        "--db-user",
        db.user.as_str(),
        "--db-password",
        db.password.as_str(),
        "--db-name",
        "sp_cov_setup_wizard",
        "--anthropic-key",
        "sk-ant-cov",
        "--openai-key",
        "sk-oai-cov",
        "--default-provider",
        "anthropic",
        "--admin-email",
        "cov-admin@example.com",
        "--no-migrate",
    ];
    let mut cmd = setup_cmd(project.path(), &base);
    cmd.assert().success();

    let profile = project
        .path()
        .join(".systemprompt/profiles/covsetup/profile.yaml");
    assert!(profile.exists(), "profile.yaml written by setup");

    let mut rerun = setup_cmd(project.path(), &base);
    rerun.assert().success();

    let mut forced_args = base.to_vec();
    forced_args.push("--force");
    let mut forced = setup_cmd(project.path(), &forced_args);
    forced.assert().success();
}

#[test]
fn setup_json_output_dry_run() {
    full_fixture();
    let project = project_dir();
    let mut cmd = setup_cmd(
        project.path(),
        &[
            "--json",
            "admin",
            "setup",
            "--dry-run",
            "-y",
            "--environment",
            "covsetup",
            "--db-host",
            "127.0.0.1",
            "--db-port",
            "9",
            "--gemini-key",
            "gm-cov",
            "--no-migrate",
        ],
    );
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("covsetup"));
}
