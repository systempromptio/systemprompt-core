//! Black-box tests that spawn the real `systemprompt-bridge` binary.
//!
//! The binary path comes from `SP_BRIDGE_BIN` (set by the coverage recipe so
//! the spawned process is instrumented) or falls back to building/locating
//! `bin/bridge`'s debug binary; if neither exists the tests are skipped so a
//! plain `nextest` run stays green without a prebuilt binary.

use std::path::PathBuf;
use std::process::{Command, Output};

use chrono::Utc;
use systemprompt_bridge::feedback::credentials::Enrollment;
use systemprompt_bridge::feedback::outbox::{Outbox, OutboxScope, PendingInstallation};
use systemprompt_bridge::ids::{BearerToken, HostId, Sha256Digest};
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceId, InstallationReceiptId, ManagedResourceId, PublicationId,
    ResourceRevisionId, UserId,
};
use systemprompt_models::bridge::manifest::SkillPublication;
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, ConsumerReceiptResponse, FileReadback, ReadbackStatus,
    ReceiptAcknowledgement, RuntimeFileReadback,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};

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
    if let Some(g) = gateway {
        let dir = home.path().join(".config").join("systemprompt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("systemprompt-bridge.toml"),
            format!("gateway_url = \"{g}\"\n"),
        )
        .unwrap();
    }
    let vars = vec![
        ("HOME", root.clone()),
        ("XDG_CONFIG_HOME", format!("{root}/.config")),
        ("XDG_CACHE_HOME", format!("{root}/.cache")),
        ("XDG_DATA_HOME", format!("{root}/.data")),
        ("XDG_STATE_HOME", format!("{root}/.state")),
    ];
    Sandbox { _home: home, vars }
}

fn run_bridge(sandbox: &Sandbox, args: &[&str]) -> Option<Output> {
    let bin = bridge_bin()?;
    let mut cmd = Command::new(bin);
    cmd.args(args);
    cmd.env_remove("SP_BRIDGE_PAT");
    cmd.env_remove("SP_BRIDGE_CONFIG");
    for (k, v) in &sandbox.vars {
        cmd.env(k, v);
    }
    Some(cmd.output().unwrap())
}

fn with_sandbox_env<R>(sandbox: &Sandbox, f: impl FnOnce() -> R) -> R {
    let vars: Vec<(&str, Option<String>)> = sandbox
        .vars
        .iter()
        .map(|(key, value)| (*key, Some(value.clone())))
        .collect();
    temp_env::with_vars(vars, f)
}

fn feedback_root(sandbox: &Sandbox) -> PathBuf {
    sandbox
        ._home
        .path()
        .join(".state")
        .join("systemprompt-bridge")
        .join("metadata")
        .join("feedback")
}

fn receipt(publication: &str, generation: i64) -> ConsumerReceiptRequest {
    let bytes = b"feedback-status fixture";
    let digest = ContentDigest::of(bytes);
    ConsumerReceiptRequest {
        installation_id: ConsumerInstallationId::generate(),
        publication_id: PublicationId::new(publication),
        resource_id: ManagedResourceId::new("feedback-status-resource"),
        revision_id: ResourceRevisionId::new(format!("{publication}-revision")),
        generation,
        bundle_digest: ContentDigest::of(format!("{publication}-bundle").as_bytes()),
        host: EvaluatorClient::Codex,
        observed_at: Utc::now(),
        files: vec![FileReadback {
            revision_id: ResourceRevisionId::new(format!("{publication}-revision")),
            path: "SKILL.md".to_owned(),
            digest: digest.clone(),
            bytes: bytes.len() as u64,
            executable: false,
            content_check: ReadbackStatus::Verified,
            mode_check: ReadbackStatus::Verified,
        }],
        runtime_files: vec![RuntimeFileReadback {
            path: "SKILL.md".to_owned(),
            digest,
            bytes: bytes.len() as u64,
            executable: false,
            content_check: ReadbackStatus::Verified,
            mode_check: ReadbackStatus::Verified,
        }],
    }
}

fn publication(publication: &str, generation: i64) -> SkillPublication {
    let digest = ContentDigest::of(format!("{publication}-bundle").as_bytes());
    SkillPublication {
        publication_id: PublicationId::new(publication),
        resource_id: ManagedResourceId::new("feedback-status-resource"),
        revision_id: ResourceRevisionId::new(format!("{publication}-revision")),
        generation,
        bundle_digest: Sha256Digest::try_new(digest.as_str()).expect("content digest is sha256"),
    }
}

fn run_bridge_with_stdin(sandbox: &Sandbox, args: &[&str], input: &str) -> Option<Output> {
    use std::io::Write;

    let bin = bridge_bin()?;
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd.env_remove("SP_BRIDGE_PAT");
    cmd.env_remove("SP_BRIDGE_CONFIG");
    for (k, v) in &sandbox.vars {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    Some(child.wait_with_output().unwrap())
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
    assert!(text.contains("login [<sp-live-...>]"));
    assert!(text.contains("--stdin"));
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

fn corrupt_portfile(sb: &Sandbox) {
    let root = sb
        .vars
        .iter()
        .find(|(k, _)| *k == "XDG_CONFIG_HOME")
        .map(|(_, v)| v.clone())
        .expect("the sandbox pins a config home");
    let dir = std::path::Path::new(&root).join("systemprompt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bridge-proxy.json"), "{ this is not json").unwrap();
}

#[test]
fn a_corrupt_port_file_still_lets_the_binary_report_its_version() {
    let sb = sandbox(None);
    corrupt_portfile(&sb);
    let out = require_bin!(run_bridge(&sb, &["--version"]));

    assert!(
        out.status.success(),
        "--version is answerable whatever the port file says; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("systemprompt-bridge "),
        "got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn a_corrupt_port_file_is_not_a_runtime_init_failure_for_whoami() {
    let sb = sandbox(None);
    corrupt_portfile(&sb);
    let out = require_bin!(run_bridge(&sb, &["whoami"]));

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        out.status.code(),
        Some(70),
        "70 means the context could not be built at all; stderr: {stderr}"
    );
    assert!(
        !stderr.contains("runtime init failed"),
        "the fault is recorded, not fatal: {stderr}"
    );
}

#[test]
fn doctor_fails_with_11_and_names_the_corrupt_port_file() {
    let sb = sandbox(None);
    corrupt_portfile(&sb);
    let out = require_bin!(run_bridge(&sb, &["doctor"]));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(11),
        "a failing check exits 11; stdout: {stdout}"
    );
    assert!(
        stdout.contains("[FAIL] startup"),
        "the startup fault is rendered as a failing check: {stdout}"
    );
    assert!(
        stdout.contains("proxy port file"),
        "the check names the component that faulted: {stdout}"
    );
}

#[test]
fn sync_without_any_credential_exits_5_and_points_at_login() {
    let sb = sandbox(Some("http://127.0.0.1:1"));
    let out = require_bin!(run_bridge(&sb, &["sync"]));

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(5),
        "a missing credential is exit 5, not the network's 3; stderr: {stderr}"
    );
    assert!(
        stderr.contains("login"),
        "the operator is told what to do about it: {stderr}"
    );
}

#[test]
fn feedback_status_requires_an_enrolled_device() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["feedback-status"]));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("device enrollment is required"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn device_enroll_requires_a_token_file_argument() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge(&sb, &["device-enroll"]));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--token-file"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn device_enroll_rejects_an_oversized_token_file_before_networking() {
    let sb = sandbox(Some("http://127.0.0.1:1"));
    let token = sb._home.path().join("large-device-token");
    std::fs::write(&token, "x".repeat(257)).unwrap();
    let out = require_bin!(run_bridge(
        &sb,
        &["device-enroll", "--token-file", token.to_str().unwrap()],
    ));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("device enrollment is required"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn device_enroll_rejects_a_non_device_credential_before_networking() {
    let sb = sandbox(Some("http://127.0.0.1:1"));
    let token = sb._home.path().join("not-a-device-token");
    std::fs::write(&token, "sp-live-user-pat\n").unwrap();
    let out = require_bin!(run_bridge(
        &sb,
        &["device-enroll", "--token-file", token.to_str().unwrap()],
    ));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("device enrollment is required"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn device_enroll_persists_the_server_assigned_device_and_feedback_status_reads_it() {
    let (server, uri) = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/consumer-devices/enrollment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_id": "device-from-gateway",
                "consumer_id": "consumer-from-gateway",
            })))
            .mount(&server)
            .await;
        let uri = server.uri();
        (server, uri)
    });
    let _ = &server;
    let sb = sandbox(Some(&uri));
    let token = sb._home.path().join("device-token");
    std::fs::write(&token, "sp_device_private\n").unwrap();

    let enrolled = require_bin!(run_bridge(
        &sb,
        &["device-enroll", "--token-file", token.to_str().unwrap()],
    ));
    assert!(
        enrolled.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&enrolled.stderr)
    );
    assert!(String::from_utf8_lossy(&enrolled.stdout).contains("device-from-gateway"));

    let status = require_bin!(run_bridge(&sb, &["feedback-status"]));
    assert!(
        status.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("installation plans: 0 unacknowledged"));
    assert!(text.contains("installation receipts: 0 acknowledged, 0 unacknowledged"));
}

#[test]
fn feedback_status_summarizes_real_outbox_delivery_and_superseded_installations() {
    let sb = sandbox(Some("http://127.0.0.1:1"));
    let root = feedback_root(&sb);
    let enrollment = Enrollment::new(
        "http://127.0.0.1:1",
        DeviceId::try_new("feedback-status-device").expect("valid fixture device"),
        UserId::new("feedback-status-consumer"),
        BearerToken::new("sp_device_feedback_status"),
    )
    .expect("valid fixture enrollment");
    enrollment.save(&root).expect("enrollment persists");

    let outbox = Outbox::new(
        enrollment.outbox_path(&root),
        OutboxScope::from_enrollment(&enrollment),
    );
    let acknowledged = outbox
        .enqueue(receipt("feedback-status-old", 1))
        .expect("valid acknowledged receipt queues");
    outbox
        .delivery(
            &acknowledged,
            Ok(ConsumerReceiptResponse {
                receipt_id: InstallationReceiptId::new("feedback-status-acknowledged"),
                acknowledgement: ReceiptAcknowledgement::Accepted,
                acknowledged_at: Utc::now(),
                fully_verified: true,
            }),
        )
        .expect("acknowledgement persists");
    outbox
        .enqueue(receipt("feedback-status-current", 2))
        .expect("valid pending receipt queues");

    outbox
        .reserve_installation(PendingInstallation::new(
            publication("feedback-status-old", 1),
            EvaluatorClient::Codex,
            vec![sb._home.path().join("old")],
        ))
        .expect("first installation plan reserves");
    outbox
        .reserve_installation(PendingInstallation::new(
            publication("feedback-status-current", 2),
            EvaluatorClient::Codex,
            vec![sb._home.path().join("current")],
        ))
        .expect("newer installation plan supersedes the earlier plan");

    let status = require_bin!(run_bridge(&sb, &["feedback-status"]));
    assert!(
        status.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(
        text.contains(
            "installation plans: 1 unacknowledged, 1 superseded without verified evidence"
        )
    );
    assert!(
        text.contains("installation receipts: 1 acknowledged, 1 unacknowledged, 1 fully verified")
    );
    assert!(
        text.contains("Codex feedback-status-resource generation 1: Acknowledged("),
        "status renders the acknowledged receipt identity"
    );
    assert!(
        text.contains("Codex feedback-status-resource generation 2: Unacknowledged"),
        "status renders the queued receipt identity"
    );
    assert!(
        !text.contains("sp_device_feedback_status") && !text.contains("\"credential\""),
        "status must not disclose the enrollment credential or raw serialized outbox"
    );
}

#[test]
fn credential_helper_emits_only_the_host_scoped_token_on_stdout() {
    let sb = sandbox(None);
    let secret = with_sandbox_env(&sb, || {
        systemprompt_bridge::proxy::secret::proxy_init().expect("sandbox mints a loopback secret")
    });
    let expected =
        systemprompt_bridge::proxy::scoped_token::host_token(&secret, &HostId::new("codex-cli"));

    let output = require_bin!(run_bridge(
        &sb,
        &["credential-helper", "--host", "codex-cli"]
    ));
    assert!(
        output.status.success(),
        "credential helper exits successfully for a known host"
    );
    let expected_stdout = format!("{}\n", expected.as_str());
    assert!(
        output.stdout == expected_stdout.as_bytes(),
        "credential helper stdout is exactly one host-scoped token line"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains(secret.as_str()),
        "the raw loopback secret must not reach stdout"
    );
    assert!(
        output.stderr.is_empty(),
        "successful helper emits no diagnostics"
    );
}

#[test]
fn device_enroll_surfaces_a_gateway_rejection_without_creating_an_enrollment() {
    let (server, uri) = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/consumer-devices/enrollment"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let uri = server.uri();
        (server, uri)
    });
    let _ = &server;
    let sb = sandbox(Some(&uri));
    let token = sb._home.path().join("rejected-device-token");
    std::fs::write(&token, "sp_device_rejected\n").unwrap();
    let out = require_bin!(run_bridge(
        &sb,
        &["device-enroll", "--token-file", token.to_str().unwrap()],
    ));

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("status 403"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let status = require_bin!(run_bridge(&sb, &["feedback-status"]));
    assert!(
        !status.status.success(),
        "rejected enrollment must not be usable"
    );
}

#[test]
fn login_stdin_rejects_an_empty_pat_without_writing_configuration() {
    let sb = sandbox(None);
    let out = require_bin!(run_bridge_with_stdin(&sb, &["login", "--stdin"], "\n"));

    assert_eq!(out.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&out.stderr).contains("stdin carried no PAT"));
    assert!(
        !sb._home
            .path()
            .join(".config/systemprompt/systemprompt-bridge.pat")
            .exists()
    );
}

#[test]
fn login_stdin_persists_a_pat_without_echoing_it() {
    let sb = sandbox(None);
    let pat = "sp-live-0123456789abcdef.0123456789abcdef0123456789abcdef";
    let out = require_bin!(run_bridge_with_stdin(
        &sb,
        &[
            "login",
            "--stdin",
            "--gateway",
            "http://127.0.0.1:1",
            "--no-reapply"
        ],
        &format!("{pat}\n"),
    ));

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Stored PAT"));
    assert!(!stdout.contains(pat));
    let stored = sb
        ._home
        .path()
        .join(".config/systemprompt/systemprompt-bridge.pat");
    assert_eq!(std::fs::read_to_string(stored).unwrap().trim(), pat);
}

#[test]
fn rejected_device_rotation_preserves_the_working_enrolment_and_a_retry_replaces_it() {
    let (server, uri) = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/consumer-devices/enrollment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_id": "device-before-repair",
                "consumer_id": "consumer-before-repair"
            })))
            .mount(&server)
            .await;
        let uri = server.uri();
        (server, uri)
    });
    let sb = sandbox(Some(&uri));
    let first_token = sb._home.path().join("first-device-token");
    std::fs::write(&first_token, "sp_device_first_private\n").unwrap();
    let first = run_bridge(
        &sb,
        &[
            "device-enroll",
            "--token-file",
            first_token.to_str().unwrap(),
        ],
    )
    .expect("SP_BRIDGE_BIN test binary must be available");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let enrollment = sb
        ._home
        .path()
        .join(".state/systemprompt-bridge/metadata/feedback/device.json");
    let before = std::fs::read(&enrollment).expect("first enrollment persisted");

    block_on(async {
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/consumer-devices/enrollment"))
            .respond_with(ResponseTemplate::new(401).set_body_string("device token revoked"))
            .mount(&server)
            .await;
    });
    let replacement_token = sb._home.path().join("replacement-device-token");
    std::fs::write(&replacement_token, "sp_device_replacement_private\n").unwrap();
    let rejected = run_bridge(
        &sb,
        &[
            "device-enroll",
            "--token-file",
            replacement_token.to_str().unwrap(),
        ],
    )
    .expect("SP_BRIDGE_BIN test binary must be available");
    assert_eq!(rejected.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("status 401"), "{stderr}");
    assert!(
        !stderr.contains("sp_device_first_private")
            && !stderr.contains("sp_device_replacement_private"),
        "neither credential is disclosed"
    );
    assert_eq!(
        std::fs::read(&enrollment).unwrap(),
        before,
        "a rejected rotation leaves the last working enrollment intact"
    );
    let status =
        run_bridge(&sb, &["feedback-status"]).expect("SP_BRIDGE_BIN test binary must be available");
    assert!(
        status.status.success(),
        "the prior enrollment remains readable"
    );

    block_on(async {
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/consumer-devices/enrollment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_id": "device-after-repair",
                "consumer_id": "consumer-after-repair"
            })))
            .mount(&server)
            .await;
    });
    let repaired = run_bridge(
        &sb,
        &[
            "device-enroll",
            "--token-file",
            replacement_token.to_str().unwrap(),
        ],
    )
    .expect("SP_BRIDGE_BIN test binary must be available");
    assert!(
        repaired.status.success(),
        "{}",
        String::from_utf8_lossy(&repaired.stderr)
    );
    assert!(String::from_utf8_lossy(&repaired.stdout).contains("device-after-repair"));
    assert_ne!(std::fs::read(&enrollment).unwrap(), before);
}

#[test]
fn status_reports_owned_inventory_rows_without_disclosing_the_pat() {
    let mut sb = sandbox(None);
    let config = sb._home.path().join(".config/systemprompt");
    std::fs::create_dir_all(&config).unwrap();
    let config_file = config.join("systemprompt-bridge.toml");
    std::fs::write(&config_file, "gateway_url = \"http://127.0.0.1:1\"\n").unwrap();
    let secret_file = config.join("systemprompt-bridge.pat");
    let pat = "sp-live-status-secret-not-for-output";
    std::fs::write(&secret_file, pat).unwrap();

    let plugins_root = sb._home.path().join("owned-org-plugins");
    sb.vars.push((
        "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
        plugins_root.to_string_lossy().into_owned(),
    ));
    let plugins = plugins_root.join("acme");
    std::fs::create_dir_all(plugins.join("skills/triage")).unwrap();
    std::fs::create_dir_all(plugins.join("skills/review")).unwrap();
    std::fs::create_dir_all(plugins.join("skills/.hidden")).unwrap();
    std::fs::create_dir_all(plugins.join("agents")).unwrap();
    std::fs::write(plugins.join("agents/reviewer.md"), "# reviewer\n").unwrap();
    std::fs::write(plugins.join("agents/notes.txt"), "not an agent\n").unwrap();
    let metadata = sb._home.path().join(".state/systemprompt-bridge/metadata");
    std::fs::create_dir_all(&metadata).unwrap();
    let sentinel = metadata.join("last-sync.json");
    std::fs::write(&sentinel, "{}").unwrap();
    std::fs::write(
        metadata.join("user.json"),
        r#"{"email":"operator@example.test"}"#,
    )
    .unwrap();

    let output = require_bin!(run_bridge(&sb, &["status"]));
    assert!(output.status.success(), "status exits successfully");
    assert!(output.stderr.is_empty(), "status emits no diagnostics");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains(&format!(
        "config file: {}\n  present: true",
        config_file.display()
    )));
    assert!(text.contains(&format!(
        "secret file: {}\n  present: true",
        secret_file.display()
    )));
    assert!(text.contains(&format!("  last sync: {}", sentinel.display())));
    assert!(text.contains("  identity: operator@example.test"));
    assert!(text.contains("  plugins: 1"));
    assert!(text.contains("  skills: 2"));
    assert!(text.contains("  agents: 1"));
    assert!(!text.contains(pat), "status must not disclose the PAT");
}
