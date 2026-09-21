//! Public service cleanup previews and confirms termination of only an owned
//! marked child.

#![cfg(unix)]

use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::infrastructure::services::{self, ServicesCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat, ScriptedPrompter};
use systemprompt_database::CreateServiceInput;
use systemprompt_models::subprocess::{AGENT_NAME_ENV, SUBPROCESS_MARKER_ENV};
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context, install_test_signing_key,
};

const HELPER: &str =
    "commands::infrastructure::services_cleanup_owned_lifecycle::cleanup_owned_helper";
const SERVICE: &str = "owned_cleanup_agent";
#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: ServicesCommands,
}
fn parse(args: &[&str]) -> ServicesCommands {
    Harness::try_parse_from(std::iter::once("services").chain(args.iter().copied()))
        .expect("parse services")
        .command
}

struct OwnedChild(Option<Child>);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[tokio::test]
#[ignore = "re-executed by dry_run_and_cancellation_preserve_owned_service_before_confirmed_cleanup"]
async fn cleanup_owned_helper() {
    let database = DisposableDb::installed("cli_services_cleanup_owned")
        .await
        .expect("private database");
    // SAFETY: the ignored helper is process-isolated and configuration is not
    // initialized.
    unsafe {
        std::env::set_var("DATABASE_URL", database.url());
        std::env::set_var("TEST_DATABASE_URL", database.url());
    }
    ensure_test_bootstrap();
    install_test_signing_key();
    let shim = tempfile::tempdir().expect("command shim");
    let lsof = shim.path().join("lsof");
    let pkill = shim.path().join("pkill");
    let pkill_log = shim.path().join("pkill.log");
    std::fs::write(&lsof, "#!/bin/sh\nexit 1\n").expect("lsof shim");
    std::fs::write(
        &pkill,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 1\n",
            pkill_log.display()
        ),
    )
    .expect("pkill shim");
    std::fs::set_permissions(&lsof, std::fs::Permissions::from_mode(0o700))
        .expect("lsof executable");
    std::fs::set_permissions(&pkill, std::fs::Permissions::from_mode(0o700))
        .expect("pkill executable");
    let mut paths = vec![shim.path().to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").expect("PATH"),
    ));
    // SAFETY: this ignored helper owns its process environment.
    unsafe {
        std::env::set_var("PATH", std::env::join_paths(paths).expect("shim PATH"));
    }
    let child = Command::new("sleep")
        .arg("60")
        .env(SUBPROCESS_MARKER_ENV, "1")
        .env(AGENT_NAME_ENV, SERVICE)
        .spawn()
        .expect("owned marked child");
    let pid = child.id();
    let mut child = OwnedChild(Some(child));
    let pool = database.pool().await.expect("private pool");
    let app = fixture_app_context(&pool, database.url()).expect("full app context");
    let repo = app.service_repository().clone();
    repo.create_service(CreateServiceInput {
        name: SERVICE,
        module_name: "agent",
        status: "running",
        port: 0,
        binary_mtime: None,
    })
    .await
    .expect("service row");
    repo.update_service_pid(SERVICE, i32::try_from(pid).expect("PID range"))
        .await
        .expect("service PID");
    let base = CliConfig::new().with_output_format(OutputFormat::Json);
    let dry = CommandContext::with_app_context(
        base.clone().with_interactive(false),
        EnvOverrides::default(),
        app.clone(),
    );
    println!("BEGIN_DRY");
    services::execute(parse(&["cleanup", "--dry-run"]), &dry)
        .await
        .expect("dry run");
    println!("END_DRY");
    assert!(
        child
            .0
            .as_mut()
            .expect("child")
            .try_wait()
            .expect("poll after dry run")
            .is_none()
    );
    assert_eq!(
        repo.find_service_by_name(SERVICE)
            .await
            .expect("row after dry run")
            .expect("row")
            .status,
        "running"
    );
    let cancel = CommandContext::with_app_context(
        base.with_interactive(true).with_assume_terminal(true),
        EnvOverrides::default(),
        app,
    )
    .with_prompter(Box::new(ScriptedPrompter::new(["no"])));
    let cancelled = services::execute(parse(&["cleanup"]), &cancel)
        .await
        .expect_err("cancel cleanup");
    println!("CANCELLED={}", serde_json::json!(format!("{cancelled:#}")));
    assert!(
        child
            .0
            .as_mut()
            .expect("child")
            .try_wait()
            .expect("poll after cancel")
            .is_none()
    );
    assert_eq!(
        repo.find_service_by_name(SERVICE)
            .await
            .expect("row after cancel")
            .expect("row")
            .status,
        "running"
    );
    println!("BEGIN_CONFIRMED");
    services::execute(parse(&["cleanup", "--yes"]), &dry)
        .await
        .expect("confirmed cleanup");
    println!("END_CONFIRMED");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child
            .0
            .as_mut()
            .expect("child")
            .try_wait()
            .expect("poll terminated")
            .is_some()
        {
            break;
        }
        assert!(Instant::now() < deadline, "owned child not terminated");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    child.0.take();
    assert_eq!(
        repo.find_service_by_name(SERVICE)
            .await
            .expect("row after cleanup")
            .expect("row")
            .status,
        "stopped"
    );
    assert!(
        !pkill_log.exists(),
        "unsafe API pattern must be rejected before pkill"
    );
    drop(cancel);
    drop(dry);
    drop(repo);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let mut out = tempfile::NamedTempFile::new().expect("stdout");
    let mut err = tempfile::NamedTempFile::new().expect("stderr");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(out.reopen().expect("stdout writer")))
        .stderr(Stdio::from(err.reopen().expect("stderr writer")));
    command.process_group(0);
    let mut child = command.spawn().expect("spawn helper");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = 'poll: loop {
        if let Some(s) = child.try_wait().expect("poll helper") {
            break s;
        }
        if Instant::now() >= deadline {
            let group = format!("-{}", child.id());
            let _ = Command::new("kill").args(["-TERM", &group]).status();
            let grace = Instant::now() + Duration::from_millis(250);
            loop {
                if let Some(s) = child.try_wait().expect("poll terminated helper") {
                    break 'poll s;
                }
                if Instant::now() >= grace {
                    let _ = Command::new("kill").args(["-KILL", &group]).status();
                    break 'poll child.wait().expect("reap helper");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let read = |f: &mut tempfile::NamedTempFile| {
        f.seek(SeekFrom::Start(0)).expect("rewind");
        let mut b = Vec::new();
        f.read_to_end(&mut b).expect("read");
        b
    };
    Output {
        status,
        stdout: read(&mut out),
        stderr: read(&mut err),
    }
}
fn marked<'a>(s: &'a str, b: &str, e: &str) -> serde_json::Value {
    serde_json::from_str(
        s.split_once(b)
            .and_then(|(_, t)| t.split_once(e))
            .map(|(v, _)| v.trim())
            .expect("markers"),
    )
    .expect("artifact")
}
fn field<'a>(v: &'a serde_json::Value, h: &str) -> &'a serde_json::Value {
    &v["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .find(|s| s["heading"] == h)
        .unwrap_or_else(|| panic!("missing {h}: {v}"))["content"]
}
#[test]
fn dry_run_and_cancellation_preserve_owned_service_before_confirmed_cleanup() {
    let mut cmd = Command::new(std::env::current_exe().expect("unit binary"));
    cmd.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(cmd);
    let sanitize = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .split_whitespace()
            .map(|word| {
                if word.contains("postgres://") || word.contains("postgresql://") {
                    "<redacted-database-url>"
                } else {
                    word
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert!(
        output.status.success(),
        "cleanup helper failed\nstdout: {}\nstderr: {}",
        sanitize(&output.stdout),
        sanitize(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8");
    let dry = marked(&stdout, "BEGIN_DRY", "END_DRY");
    assert_eq!(field(&dry, "services_cleaned"), 1);
    assert_eq!(field(&dry, "dry_run"), true);
    assert!(
        stdout.contains("CANCELLED=\"Operation cancelled"),
        "{stdout}"
    );
    let done = marked(&stdout, "BEGIN_CONFIRMED", "END_CONFIRMED");
    assert_eq!(field(&done, "services_cleaned"), 1);
    assert_eq!(field(&done, "dry_run"), false);
}
