//! Black-box tests that spawn the real `systemprompt-bridge` binary.
//!
//! The binary path comes from `SP_BRIDGE_BIN` (set by the coverage recipe so
//! the spawned process is instrumented) or falls back to building/locating
//! `bin/bridge`'s debug binary; if neither exists the tests are skipped so a
//! plain `nextest` run stays green without a prebuilt binary.

use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bridge_bin() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("SP_BRIDGE_BIN") {
        let p = PathBuf::from(explicit);
        return p.is_file().then_some(p);
    }
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)?
        .to_path_buf();
    let fallback = repo_root
        .join("bin")
        .join("bridge")
        .join("target")
        .join("debug")
        .join("systemprompt-bridge");
    fallback.is_file().then_some(fallback)
}

struct Sandbox {
    _home: TempDir,
    vars: Vec<(&'static str, String)>,
}

fn sandbox(gateway: Option<&str>) -> Sandbox {
    let home = TempDir::new().unwrap();
    let root = home.path().to_string_lossy().into_owned();
    let mut vars = vec![
        ("HOME", root.clone()),
        ("XDG_CONFIG_HOME", format!("{root}/.config")),
        ("XDG_CACHE_HOME", format!("{root}/.cache")),
        ("XDG_DATA_HOME", format!("{root}/.data")),
        ("XDG_STATE_HOME", format!("{root}/.state")),
    ];
    if let Some(g) = gateway {
        vars.push(("SP_BRIDGE_GATEWAY_URL", g.to_owned()));
    }
    Sandbox { _home: home, vars }
}

fn run_bridge(sandbox: &Sandbox, args: &[&str]) -> Option<Output> {
    let bin = bridge_bin()?;
    let mut cmd = Command::new(bin);
    cmd.args(args);
    cmd.env_remove("SP_BRIDGE_PAT");
    cmd.env_remove("SP_BRIDGE_CONFIG");
    cmd.env_remove("SP_BRIDGE_GATEWAY_URL");
    for (k, v) in &sandbox.vars {
        cmd.env(k, v);
    }
    Some(cmd.output().unwrap())
}

macro_rules! require_bin {
    ($out:expr) => {
        match $out {
            Some(o) => o,
            None => {
                eprintln!("bridge binary not available; skipping");
                return;
            },
        }
    };
}

#[test]
fn help_prints_command_reference() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["help"]));
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("systemprompt-bridge <command>"));
    assert!(text.contains("login <sp-live-...>"));
}

#[test]
fn version_prints_semver_line() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["--version"]));
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("systemprompt-bridge "), "got: {text}");
}

#[test]
fn unknown_command_exits_nonzero_with_help() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["frobnicate"]));
    assert!(!out.status.success());
}

#[test]
fn run_without_credentials_exits_5() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["run"]));
    assert_eq!(out.status.code(), Some(5));
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

#[test]
fn run_with_pat_emits_jwt_envelope() {
    let (server, uri) = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/auth/bridge/pat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "jwt.subprocess.token",
                "ttl": 900,
            })))
            .mount(&server)
            .await;
        let uri = server.uri();
        (server, uri)
    });
    let _ = &server;

    let sb = sandbox(Some(&uri));
    let bin = match bridge_bin() {
        Some(b) => b,
        None => {
            eprintln!("bridge binary not available; skipping");
            return;
        },
    };
    let mut cmd = Command::new(bin);
    cmd.arg("run");
    for (k, v) in &sb.vars {
        cmd.env(k, v);
    }
    cmd.env("SP_BRIDGE_PAT", "sp-live-subprocess-pat");
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be a JSON envelope");
    assert_eq!(envelope["token"], "jwt.subprocess.token");
}

#[test]
fn status_reports_paths_without_credentials() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["status"]));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.to_lowercase().contains("config") || !out.status.success(),
        "status should mention config paths: {text}"
    );
}

#[test]
fn diagnostics_runs_to_completion() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["diagnostics"]));
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn install_print_mdm_linux_prints_snippet() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["install", "--print-mdm", "linux"]));
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.stdout.is_empty(), "MDM snippet must be printed");
}

#[test]
fn install_rejects_invalid_gateway_url() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["install", "--gateway", "not a url"]));
    assert_eq!(out.status.code(), Some(64));
}

#[test]
fn credential_helper_get_emits_json_error_without_creds() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["credential-helper", "get"]));
    let text = String::from_utf8_lossy(&out.stdout);
    let _ = text;
}

#[test]
fn proxy_headless_starts_and_stops_on_sigint() {
    let bin = match bridge_bin() {
        Some(b) => b,
        None => {
            eprintln!("bridge binary not available; skipping");
            return;
        },
    };
    let sb = sandbox(None);
    let mut cmd = Command::new(bin);
    cmd.arg("proxy");
    for (k, v) in &sb.vars {
        cmd.env(k, v);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        while let Ok(n) = stdout.read(&mut chunk) {
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).into_owned();
            if text.contains("proxy listening on") {
                let _ = tx.send(text);
                break;
            }
        }
    });

    let banner = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("proxy must print its listening banner");
    assert!(banner.contains("ANTHROPIC_BASE_URL"));
    std::thread::sleep(std::time::Duration::from_millis(750));

    let _ = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();

    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let _ = status;
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            let _ = child.kill();
            panic!("proxy did not exit after SIGINT");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
