use clap::Parser;
use std::ffi::OsString;
use std::path::PathBuf;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, ScriptedPrompter};

struct Sandbox {
    root: tempfile::TempDir,
    previous: PathBuf,
    path: Option<OsString>,
}
impl Sandbox {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".systemprompt/docker")).unwrap();
        std::fs::create_dir(root.path().join("bin")).unwrap();
        std::fs::write(root.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let previous = std::env::current_dir().unwrap();
        let path = std::env::var_os("PATH");
        std::env::set_current_dir(root.path()).unwrap();
        // SAFETY: nextest isolates each test in a process; child commands use only this
        // sandbox.
        unsafe {
            std::env::set_var("PATH", root.path().join("bin"));
        }
        Self {
            root,
            previous,
            path,
        }
    }
    fn executable(&self, name: &str, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.path().join("bin").join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn calls(&self) -> String {
        std::fs::read_to_string(self.root.path().join("calls")).unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).unwrap();
        // SAFETY: restores the process-local environment installed by this test.
        unsafe {
            match &self.path {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
        }
    }
}
#[derive(Parser)]
struct Build {
    #[command(subcommand)]
    command: systemprompt_cli::build::BuildCommands,
}
fn build(args: &[&str]) -> anyhow::Result<()> {
    let command = Build::try_parse_from(std::iter::once("build").chain(args.iter().copied()))
        .unwrap()
        .command;
    systemprompt_cli::build::execute(
        command,
        &CommandContext::new(
            CliConfig::new().with_interactive(false),
            EnvOverrides::default(),
        ),
    )
}
#[test]
fn coverage_core_build_forwards_release_and_offline_to_the_project_workspace() {
    let sandbox = Sandbox::new();
    sandbox.executable(
        "cargo",
        "printf '%s\\n' \"$PWD\" \"$*\" \"$SQLX_OFFLINE\" > calls",
    );
    build(&["core", "--release", "--offline"]).unwrap();
    let calls = sandbox.calls();
    assert!(calls.contains(sandbox.root.path().to_str().unwrap()));
    assert!(
        calls.contains("build --workspace --release\ntrue"),
        "{calls}"
    );
}
#[test]
fn coverage_core_debug_build_does_not_enable_release() {
    let sandbox = Sandbox::new();
    sandbox.executable("cargo", "printf '%s\\n' \"$*\" > calls");
    build(&["core"]).unwrap();
    assert_eq!(sandbox.calls().trim(), "build --workspace");
}
#[test]
fn coverage_core_build_reports_failed_compilation() {
    let sandbox = Sandbox::new();
    sandbox.executable("cargo", "exit 17");
    assert!(format!("{:#}", build(&["core"]).unwrap_err()).contains("Cargo build failed"));
}
#[test]
fn coverage_core_build_reports_missing_cargo() {
    let _sandbox = Sandbox::new();
    assert!(
        format!("{:#}", build(&["core"]).unwrap_err()).contains("Failed to execute cargo build")
    );
}
async fn local_setup(sandbox: &Sandbox, answers: &[&str]) -> anyhow::Result<()> {
    let compose = sandbox
        .root
        .path()
        .join(".systemprompt/docker/fixture.yaml");
    std::fs::write(compose, "services: {}\n").unwrap();
    systemprompt_cli::cloud::profile::handle_local_tenant_setup(
        &ScriptedPrompter::new(answers.iter().copied()),
        "postgres://unused:unused@127.0.0.1:1/unused",
        "fixture",
        &sandbox.root.path().join("profile.yaml"),
    )
    .await
}
#[tokio::test]
async fn coverage_local_setup_declining_docker_does_not_execute_it() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf called > calls; exit 1");
    local_setup(&sandbox, &["no"]).await.unwrap();
    assert!(!sandbox.root.path().join("calls").exists());
}
#[tokio::test]
async fn coverage_local_setup_failed_docker_start_does_not_ask_for_migrations() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf '%s\\n' \"$*\" > calls; exit 1");
    local_setup(&sandbox, &["yes"]).await.unwrap();
    let calls = sandbox.calls();
    assert!(calls.contains("compose -p fixture -f"));
    assert!(calls.trim().ends_with("up -d"));
}
#[tokio::test]
async fn coverage_local_setup_missing_docker_has_actionable_error() {
    let sandbox = Sandbox::new();
    let err = local_setup(&sandbox, &["yes"]).await.unwrap_err();
    assert!(err.to_string().contains("Failed to execute docker compose"));
}
#[tokio::test]
async fn coverage_local_setup_waits_for_health_before_offering_migrations() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf '%s\\n' \"$*\" >> calls; case \"$*\" in *' up -d') exit 0;; *) printf healthy;; esac");
    local_setup(&sandbox, &["yes", "no"]).await.unwrap();
    let calls = sandbox.calls();
    assert_eq!(calls.lines().count(), 2, "{calls}");
    assert!(calls.contains("ps"));
}
#[tokio::test]
async fn coverage_local_setup_health_probe_failure_does_not_offer_migrations() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf '%s\\n' \"$*\" >> calls; case \"$*\" in *' up -d') /bin/rm -- \"$0\"; exit 0;; *) exit 3;; esac");
    local_setup(&sandbox, &["yes"]).await.unwrap();
    assert_eq!(sandbox.calls().lines().count(), 1);
}
