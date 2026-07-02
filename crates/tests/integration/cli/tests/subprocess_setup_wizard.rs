//! Subprocess coverage for `admin setup`: dry-run preview and the full
//! non-interactive profile/secrets generation flow in an isolated project.

use std::path::Path;

use systemprompt_cli_integration_tests::full_bootstrap::{command_bare, database_url, fixture};

struct DbParts {
    host: String,
    port: String,
    user: String,
    password: String,
}

fn db_parts() -> Option<DbParts> {
    let raw = database_url()?;
    let url = url::Url::parse(&raw).ok()?;
    Some(DbParts {
        host: url.host_str()?.to_owned(),
        port: url.port().unwrap_or(5432).to_string(),
        user: url.username().to_owned(),
        password: url.password()?.to_owned(),
    })
}

fn project_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create project dir");
    std::fs::create_dir_all(dir.path().join("services")).expect("mkdir services");
    dir
}

fn setup_cmd(project: &Path, args: &[&str]) -> Option<assert_cmd::Command> {
    let mut cmd = command_bare()?;
    cmd.env("HOME", project);
    cmd.current_dir(project);
    cmd.args(args);
    Some(cmd)
}

#[test]
fn setup_dry_run_previews_without_writing() {
    if fixture().is_none() {
        return;
    }
    let project = project_dir();
    let Some(mut cmd) = setup_cmd(
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
    ) else {
        return;
    };
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
    let Some(db) = db_parts() else { return };
    if fixture().is_none() {
        return;
    }
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
        "--no-migrate",
    ];
    let Some(mut cmd) = setup_cmd(project.path(), &base) else {
        return;
    };
    cmd.assert().success();

    let profile = project
        .path()
        .join(".systemprompt/profiles/covsetup/profile.yaml");
    assert!(profile.exists(), "profile.yaml written by setup");

    let Some(mut rerun) = setup_cmd(project.path(), &base) else {
        return;
    };
    rerun.assert().success();

    let mut forced_args = base.to_vec();
    forced_args.push("--force");
    let Some(mut forced) = setup_cmd(project.path(), &forced_args) else {
        return;
    };
    forced.assert().success();
}

#[test]
fn setup_json_output_dry_run() {
    if fixture().is_none() {
        return;
    }
    let project = project_dir();
    let Some(mut cmd) = setup_cmd(
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
    ) else {
        return;
    };
    cmd.assert().success();
}
