use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Install {
    root: TempDir,
    binary: PathBuf,
}

impl Install {
    fn new(gateway: &str) -> Option<Self> {
        let source = if let Some(path) = std::env::var_os("SP_BRIDGE_BIN") {
            let path = PathBuf::from(path);
            assert!(path.is_file(), "SP_BRIDGE_BIN must name an existing binary");
            path
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(5)
                .unwrap()
                .join("bin/bridge/target/debug/systemprompt-bridge")
        };
        if !source.is_file() {
            eprintln!("bridge binary unavailable; set SP_BRIDGE_BIN to run update flows");
            return None;
        }
        let root = TempDir::new().unwrap();
        let binary = root.path().join("systemprompt-bridge");
        std::fs::copy(source, &binary).unwrap();
        std::fs::write(
            root.path().join("config.toml"),
            format!("gateway_url = {gateway:?}\n"),
        )
        .unwrap();
        Some(Self { root, binary })
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .stdin(Stdio::null())
            .env("HOME", self.root.path())
            .env_remove("SUDO_USER")
            .env("SP_BRIDGE_CONFIG", self.root.path().join("config.toml"))
            .env("SP_BRIDGE_PAT", "sp-live-update-fixture");
        for (name, dir) in [
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
        ] {
            command.env(name, self.root.path().join(dir));
        }
        command.output().unwrap()
    }
}

async fn gateway(release: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"token":"fixture.jwt", "ttl":900})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/latest"))
        .and(header("authorization", "Bearer fixture.jwt"))
        .respond_with(release)
        .expect(1)
        .mount(&server)
        .await;
    server
}

fn manifest(version: &str, body: &[u8]) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "version":version, "sha256":Sha256::digest(body).iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "size":body.len(), "notes_url":"https://example.invalid/release",
    }))
}

fn assert_exit(out: &Output, code: i32) {
    assert_eq!(
        out.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_flows_check_current_does_not_download() {
    let server = gateway(manifest("0.0.0", b"")).await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&["update", "--check"]);
    assert_exit(&out, 0);
    assert!(String::from_utf8_lossy(&out.stdout).contains("up to date"));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_flows_check_available_reports_release_without_installing() {
    let server = gateway(manifest("999.0.0", b"")).await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&["update", "--check"]);
    assert_exit(&out, 1);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("999.0.0") && text.contains("https://example.invalid/release"));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_flows_noninteractive_update_requires_explicit_yes() {
    let server = gateway(manifest("999.0.0", b"")).await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&["update"]);
    assert_exit(&out, 0);
    assert!(String::from_utf8_lossy(&out.stdout).contains("cancelled"));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_flows_bad_release_responses_exit_three() {
    for response in [
        ResponseTemplate::new(503),
        ResponseTemplate::new(200).set_body_string("broken"),
        manifest("not-a-version", b""),
    ] {
        let server = gateway(response).await;
        let Some(install) = Install::new(&server.uri()) else {
            return;
        };
        let out = install.run(&["update", "--check"]);
        assert_exit(&out, 3);
        assert!(String::from_utf8_lossy(&out.stderr).contains("update check failed"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_flows_download_failure_preserves_the_installed_binary() {
    let server = gateway(manifest("999.0.0", b"expected")).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"wrong".to_vec()))
        .with_priority(10)
        .mount(&server)
        .await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let before = std::fs::read(&install.binary).unwrap();
    let out = install.run(&["update", "--yes"]);
    assert_exit(&out, 3);
    assert!(String::from_utf8_lossy(&out.stderr).contains("update failed"));
    assert_eq!(std::fs::read(&install.binary).unwrap(), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_flows_verified_archive_replaces_only_the_sandbox_binary() {
    let archive_dir = TempDir::new().unwrap();
    let replacement = b"#!/bin/sh\nprintf 'updated fixture\\n'\n";
    std::fs::write(archive_dir.path().join("systemprompt-bridge"), replacement).unwrap();
    let archive = archive_dir.path().join("release.tar.gz");
    assert!(
        Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(archive_dir.path())
            .arg("systemprompt-bridge")
            .status()
            .unwrap()
            .success()
    );
    let body = std::fs::read(&archive).unwrap();
    let server = gateway(manifest("999.0.0", &body)).await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer fixture.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .with_priority(10)
        .mount(&server)
        .await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&["update", "--yes"]);
    assert_exit(&out, 0);
    assert!(String::from_utf8_lossy(&out.stdout).contains("updated to 999.0.0"));
    assert_eq!(std::fs::read(&install.binary).unwrap(), replacement);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&install.binary)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coverage_login_redeems_one_time_code_and_persists_a_private_pat_file() {
    let server = MockServer::start().await;
    let pat = "sp-live-0123456789abcdef.0123456789abcdef0123456789abcdef";
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pat":pat})))
        .expect(1)
        .mount(&server)
        .await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&[
        "login",
        "--code",
        " fixture-code ",
        "--device-name",
        "coverage-device",
        "--no-reapply",
    ]);
    assert_exit(&out, 0);
    let output = String::from_utf8_lossy(&out.stdout);
    assert!(output.contains("Stored PAT"), "{output}");
    assert!(!output.contains(pat));
    let request = &server.received_requests().await.unwrap()[0];
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["code"], "fixture-code");
    assert_eq!(body["device_name"], "coverage-device");
    let pat_path = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("secret: "))
        .unwrap()
        .strip_suffix(" (0600)")
        .unwrap();
    let pat_path = PathBuf::from(pat_path);
    assert!(pat_path.starts_with(install.root.path()));
    assert_eq!(std::fs::read_to_string(&pat_path).unwrap().trim(), pat);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(pat_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coverage_login_rejected_code_does_not_write_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(ResponseTemplate::new(403).set_body_string("expired code"))
        .expect(1)
        .mount(&server)
        .await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&["login", "--code", "expired", "--no-reapply"]);
    assert_exit(&out, 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("could not redeem"));
    assert!(
        !install
            .root
            .path()
            .join("config/systemprompt/systemprompt-bridge.pat")
            .exists()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coverage_login_pasted_pat_is_saved_without_an_exchange_or_reapply() {
    let server = MockServer::start().await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&[
        "login",
        "sp-live-0123456789abcdef.0123456789abcdef0123456789abcdef",
        "--gateway",
        &server.uri(),
    ]);
    assert_exit(&out, 0);
    assert!(String::from_utf8_lossy(&out.stdout).contains("Stored PAT"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coverage_login_rejects_interactive_sso_without_a_terminal() {
    let server = MockServer::start().await;
    let Some(install) = Install::new(&server.uri()) else {
        return;
    };
    let out = install.run(&["login"]);
    assert_exit(&out, 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("needs a terminal"));
    assert!(server.received_requests().await.unwrap().is_empty());
}
