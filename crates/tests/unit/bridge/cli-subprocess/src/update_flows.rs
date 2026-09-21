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
            assert!(
                std::env::var_os("CI").is_none(),
                "bridge binary unavailable under CI; scripts/test-shard.sh must prebuild it"
            );
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

    fn run_with_stored_pat(&self, args: &[&str]) -> Output {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .stdin(Stdio::null())
            .env("HOME", self.root.path())
            .env_remove("SUDO_USER")
            .env_remove("SP_BRIDGE_PAT")
            .env_remove("SP_BRIDGE_CONFIG");
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
    let exchanged = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .any(|request| request.url.path() == "/v1/auth/bridge/session-pat");
    assert!(
        !exchanged,
        "a pasted PAT is stored as-is; only the best-effort device enrolment may reach the gateway"
    );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_code_completes_device_enrolment_with_the_pat_it_just_stored() {
    let server = MockServer::start().await;
    let pat = "sp-live-login-lifecycle.0123456789abcdef0123456789abcdef";
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pat": pat})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .and(header("authorization", format!("Bearer {pat}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "token": "login.jwt", "ttl": 900
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/whoami"))
        .and(header("authorization", "Bearer login.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user_id": "00000000-0000-4000-8000-00000000fee1"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/bridge/device"))
        .and(header("authorization", "Bearer login.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_id": "device-login-lifecycle",
            "consumer_id": "00000000-0000-4000-8000-00000000fee1",
            "credential": "sp_device_login_lifecycle"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let install = Install::new(&server.uri())
        .expect("SP_BRIDGE_BIN test binary must be available for login lifecycle coverage");

    let output = install.run_with_stored_pat(&[
        "login",
        "--code",
        "one-shot-code",
        "--device-name",
        "test-workstation",
        "--gateway",
        &server.uri(),
        "--no-reapply",
    ]);
    assert_exit(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("device-login-lifecycle"),
        "the successful command reports the enrolled device: {stdout}"
    );
    let pat_path = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("secret: "))
        .expect("login reports the credential path")
        .strip_suffix(" (0600)")
        .expect("login reports the credential mode");
    assert_eq!(
        std::fs::read_to_string(pat_path).unwrap().trim(),
        pat,
        "post-login enrolment authenticates from the newly persisted PAT"
    );
    let requests = server.received_requests().await.unwrap();
    let enrolment = requests
        .iter()
        .find(|request| request.url.path() == "/v1/bridge/device")
        .expect("login performs device enrolment");
    let body: serde_json::Value = serde_json::from_slice(&enrolment.body).unwrap();
    let hostname = hostname::get()
        .expect("read the machine hostname")
        .into_string()
        .expect("machine hostname is UTF-8");
    assert_eq!(body["label"], hostname.trim());
    assert!(
        body["fingerprint"]
            .as_str()
            .is_some_and(|value| value.len() == 64)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_keeps_the_new_pat_when_best_effort_device_enrolment_is_rejected() {
    let server = MockServer::start().await;
    let pat = "sp-live-enrolment-failure.0123456789abcdef0123456789abcdef";
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pat": pat})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .and(header("authorization", format!("Bearer {pat}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "token": "login.failure.jwt", "ttl": 900
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user_id": "00000000-0000-4000-8000-00000000fee1"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/bridge/device"))
        .respond_with(ResponseTemplate::new(500).set_body_string("credential service unavailable"))
        .expect(1)
        .mount(&server)
        .await;
    let install = Install::new(&server.uri())
        .expect("SP_BRIDGE_BIN test binary must be available for login lifecycle coverage");

    let output = install.run_with_stored_pat(&[
        "login",
        "--code",
        "valid-one-shot-code",
        "--gateway",
        &server.uri(),
        "--no-reapply",
    ]);
    assert_exit(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("Stored PAT"), "{stdout}");
    assert!(stderr.contains("device enrolment skipped"), "{stderr}");
    assert!(
        !stdout.contains(pat) && !stderr.contains(pat),
        "the PAT must never be printed"
    );
    let pat_path = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("secret: "))
        .expect("login reports the credential path")
        .strip_suffix(" (0600)")
        .expect("login reports the credential mode");
    assert_eq!(std::fs::read_to_string(pat_path).unwrap().trim(), pat);
    assert!(
        !install
            .root
            .path()
            .join("state/systemprompt-bridge/metadata/feedback/device.json")
            .exists(),
        "a rejected enrolment does not fabricate a device credential"
    );
}
// Append to cli-subprocess/src/update_flows.rs; add `libc = { workspace = true
// }` dev-dependency.
#[cfg(unix)]
struct ChildGuard(Option<std::process::Child>);
#[cfg(unix)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[cfg(unix)]
fn read_pty_until(file: &mut std::fs::File, needle: &str) -> String {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut bytes = Vec::new();
    while std::time::Instant::now() < deadline {
        let mut poll = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll points to one live pollfd for the duration of the call.
        let ready = unsafe { libc::poll(&raw mut poll, 1, 100) };
        assert!(
            ready >= 0,
            "PTY poll failed: {}",
            std::io::Error::last_os_error()
        );
        if ready == 0 {
            continue;
        }
        let mut chunk = [0_u8; 1024];
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(error) => panic!("PTY read failed: {error}"),
        }
        let text = String::from_utf8_lossy(&bytes);
        if text.contains(needle) {
            return text.into_owned();
        }
    }
    panic!(
        "PTY output never contained {needle:?}: {}",
        String::from_utf8_lossy(&bytes)
    );
}

#[cfg(unix)]
fn read_pty_to_eof(file: &mut std::fs::File) -> String {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut bytes = Vec::new();
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "PTY did not reach EOF"
        );
        let mut poll = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN | libc::POLLHUP,
            revents: 0,
        };
        // SAFETY: poll points to one live pollfd for the duration of the call.
        let ready = unsafe { libc::poll(&raw mut poll, 1, 100) };
        assert!(
            ready >= 0,
            "PTY poll failed: {}",
            std::io::Error::last_os_error()
        );
        if ready == 0 {
            continue;
        }
        let mut chunk = [0_u8; 1024];
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(error) => panic!("PTY read failed: {error}"),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_no_browser_pastes_code_through_a_terminal_and_persists_the_pat() {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    let server = MockServer::start().await;
    let pat = "sp-live-pty-login.0123456789abcdef0123456789abcdef";
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pat":pat})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .and(header("authorization", format!("Bearer {pat}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "token": "pty.login.jwt", "ttl": 900
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/whoami"))
        .and(header("authorization", "Bearer pty.login.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user_id": "00000000-0000-4000-8000-00000000fee1"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/bridge/device"))
        .and(header("authorization", "Bearer pty.login.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_id": "device-pty-login",
            "consumer_id": "00000000-0000-4000-8000-00000000fee1",
            "credential": "sp_device_pty_login"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let install = Install::new(&server.uri()).expect("instrumented bridge binary");
    let mut master = 0;
    let mut slave = 0;
    // SAFETY: openpty initializes both descriptors; null termios/winsize request
    // defaults.
    assert_eq!(
        unsafe {
            libc::openpty(
                &raw mut master,
                &raw mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        },
        0
    );
    // SAFETY: each descriptor is uniquely transferred into one File.
    let mut master = unsafe { std::fs::File::from_raw_fd(master) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    let mut command = Command::new(&install.binary);
    command
        .args([
            "login",
            "--no-browser",
            "--gateway",
            &server.uri(),
            "--no-reapply",
        ])
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave)
        .env("HOME", install.root.path())
        .env_remove("SUDO_USER")
        .env_remove("SP_BRIDGE_PAT")
        .env_remove("SP_BRIDGE_CONFIG")
        .env("XDG_CONFIG_HOME", install.root.path().join("config"))
        .env("XDG_CACHE_HOME", install.root.path().join("cache"))
        .env("XDG_DATA_HOME", install.root.path().join("data"))
        .env("XDG_STATE_HOME", install.root.path().join("state"));
    let mut child = ChildGuard(Some(command.spawn().expect("spawn bridge under PTY")));
    drop(command);
    let prompt = read_pty_until(&mut master, "Paste it here:");
    assert!(prompt.contains("/bridge/device"), "{prompt}");
    master.write_all(b"  pty-one-time-code  \n").unwrap();
    let output = read_pty_to_eof(&mut master);
    assert!(output.contains("Stored PAT"), "{output}");
    assert!(!output.contains(pat));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.0.as_mut().unwrap().try_wait().expect("poll login") {
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "login did not exit after storing PAT"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    child.0.take();
    assert!(status.success());
    let requests = server
        .received_requests()
        .await
        .expect("recorded redemption");
    let paths: Vec<&str> = requests.iter().map(|request| request.url.path()).collect();
    for expected in [
        "/v1/auth/bridge/session-pat",
        "/v1/auth/bridge/pat",
        "/v1/bridge/whoami",
        "/v1/bridge/device",
    ] {
        assert_eq!(
            paths.iter().filter(|path| **path == expected).count(),
            1,
            "request paths: {paths:?}"
        );
    }
    assert_eq!(paths.len(), 4, "no unexpected request: {paths:?}");
    let redemption = requests
        .iter()
        .find(|request| request.url.path() == "/v1/auth/bridge/session-pat")
        .expect("one-time code redemption");
    let body: serde_json::Value = serde_json::from_slice(&redemption.body).unwrap();
    assert_eq!(body["code"], "pty-one-time-code");
    let pat_path = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("secret: "))
        .expect("secret path in login output")
        .strip_suffix(" (0600)")
        .expect("secret mode suffix");
    let pat_path = std::path::Path::new(pat_path);
    assert!(pat_path.starts_with(install.root.path()));
    assert_eq!(std::fs::read_to_string(pat_path).unwrap().trim(), pat);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(pat_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
