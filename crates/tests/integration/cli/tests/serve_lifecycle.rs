#![cfg(unix)]

use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use systemprompt_cli_integration_tests::full_bootstrap::{
    TEST_MANIFEST_SIGNING_SEED, TEST_OAUTH_AT_REST_PEPPER, isolated_fixture,
};
use systemprompt_scheduler::ProcessCleanup;
use systemprompt_test_fixtures::{DisposableDb, seed_user_row_with_roles};

const TEST_ENCRYPTION_MASTER_KEY: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";
const TEST_ANTHROPIC_KEY: &str = "sk-ant-test-owned-agent-lifecycle";

struct OwnedServer {
    child: Child,
    stderr: tempfile::NamedTempFile,
    cleanup_agent: Option<OwnedAgent>,
}

struct OwnedAgent {
    name: String,
    port: u16,
    observed_pids: Vec<u32>,
    log_file: std::path::PathBuf,
}

impl OwnedServer {
    fn stderr_text(&self) -> String {
        let stderr = std::fs::read_to_string(self.stderr.path())
            .unwrap_or_else(|error| format!("<failed to read child stderr: {error}>"));
        let Some(agent) = &self.cleanup_agent else {
            return stderr;
        };
        let agent_log = std::fs::read_to_string(&agent.log_file)
            .unwrap_or_else(|error| format!("<agent log unavailable: {error}>"));
        format!(
            "{stderr}\nagent log ({}):\n{agent_log}",
            agent.log_file.display()
        )
    }
}

impl Drop for OwnedServer {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(agent) = &self.cleanup_agent {
            let mut candidates = agent.observed_pids.clone();
            if let Some(pid) = ProcessCleanup::check_port(agent.port) {
                candidates.push(pid);
            }
            candidates.extend(agent_pids(&agent.name));
            candidates.sort_unstable();
            candidates.dedup();
            for pid in candidates {
                if systemprompt_loader::subprocess::live_pid_is_subprocess(
                    pid,
                    "AGENT_NAME",
                    &agent.name,
                ) {
                    ProcessCleanup::kill_process(pid);
                    for _ in 0..40 {
                        if !ProcessCleanup::process_exists(pid)
                            || systemprompt_loader::subprocess::is_zombie(pid)
                        {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
    }
}

fn agent_pids(name: &str) -> Vec<u32> {
    let Ok(output) = Command::new("ps").args(["-axo", "pid="]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(|value| value.parse().ok())
        .filter(|pid| {
            systemprompt_loader::subprocess::live_pid_is_subprocess(*pid, "AGENT_NAME", name)
        })
        .collect()
}

fn bounded_output(mut command: Command, label: &str) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("create bounded CLI stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("create bounded CLI stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            stdout.reopen().expect("open CLI stdout capture"),
        ))
        .stderr(Stdio::from(
            stderr.reopen().expect("open CLI stderr capture"),
        ));
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll bounded CLI child") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).expect("read bounded CLI stdout"),
                stderr: std::fs::read(stderr.path()).expect("read bounded CLI stderr"),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap timed-out CLI child");
            let output = Output {
                status,
                stdout: std::fs::read(stdout.path()).expect("read timed-out CLI stdout"),
                stderr: std::fs::read(stderr.path()).expect("read timed-out CLI stderr"),
            };
            panic!(
                "{label} timed out\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn cli(profile: &std::path::Path, database_url: &str) -> Command {
    let binary = std::env::var_os("SYSTEMPROMPT_BIN")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_file())
        .expect("SYSTEMPROMPT_BIN must name the instrumented systemprompt binary");
    let mut command = Command::new(binary);
    command
        .env_remove("RUST_LOG")
        .env_remove("SYSTEMPROMPT_PROFILE")
        .env("DATABASE_URL", database_url)
        .env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER)
        .env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED)
        .env(
            "SYSTEMPROMPT_CUSTOM_SECRETS",
            "encryption_master_key,anthropic",
        )
        .env("encryption_master_key", TEST_ENCRYPTION_MASTER_KEY)
        .env("anthropic", TEST_ANTHROPIC_KEY)
        .env("ANTHROPIC_API_KEY", TEST_ANTHROPIC_KEY)
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .args(["--non-interactive", "--no-color", "--profile"])
        .arg(profile);
    command
}

fn isolate_fixture_agent(
    fixture: &systemprompt_cli_integration_tests::full_bootstrap::FullBootstrap,
    name: &str,
) {
    let path = fixture.services_dir.join("config/config.yaml");
    let config = std::fs::read_to_string(&path).expect("read enabled agent config");
    let included_mcp = format!(
        "mcpServers:\n        source: instance\n        include:\n        - {}",
        systemprompt_cli_integration_tests::full_bootstrap::fixture_mcp_server()
    );
    let without_mcp = config.replace(
        &included_mcp,
        "mcpServers:\n        source: instance\n        include: []",
    );
    assert_ne!(
        without_mcp, config,
        "fixture agent MCP include was not present"
    );
    let renamed = without_mcp.replace("covagent", name);
    assert_ne!(renamed, without_mcp, "fixture agent name was not present");
    std::fs::write(path, renamed).expect("write uniquely named agent config");

    complete_fixture_web_paths(fixture);
}

fn complete_fixture_web_paths(
    fixture: &systemprompt_cli_integration_tests::full_bootstrap::FullBootstrap,
) {
    let web = fixture.services_dir.join("web");
    std::fs::create_dir_all(web.join("templates")).expect("create fixture web templates");
    std::fs::create_dir_all(web.join("assets")).expect("create fixture web assets");
    let web_config_path = web.join("config.yaml");
    let web_config = std::fs::read_to_string(&web_config_path).expect("read fixture web config");
    std::fs::write(
        web_config_path,
        format!(
            "paths:\n  templates: {}\n  assets: {}\n{web_config}",
            web.join("templates").display(),
            web.join("assets").display()
        ),
    )
    .expect("complete fixture web paths");
}

async fn wait_until_ready(client: &reqwest::Client, base: &str, server: &mut OwnedServer) {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = server.child.try_wait().expect("poll API child") {
            panic!(
                "API child exited before readiness: {status}\nstderr:\n{}",
                server.stderr_text()
            );
        }
        if let Ok(response) = client.get(format!("{base}/readyz")).send().await
            && response.status().is_success()
        {
            let body: serde_json::Value = response.json().await.expect("read readiness JSON");
            assert_eq!(body["status"], "ready", "{body}");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "API did not become ready at {base}\nstderr:\n{}",
            server.stderr_text()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_exit(server: &mut OwnedServer) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = server.child.try_wait().expect("poll API shutdown") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "API ignored SIGTERM\nstderr:\n{}",
            server.stderr_text()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_owned_listener(port: u16, expected_pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let holder = ProcessCleanup::check_port(port);
        if holder == Some(expected_pid) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "port {port} ownership did not settle on PID {expected_pid}; last holder: {holder:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn cli_serve_reaches_authenticated_health_and_shuts_down_gracefully() {
    let database = DisposableDb::installed("cli_serve_vertical")
        .await
        .expect("dedicated installed database");
    let pool = database.pool().await.expect("dedicated database pool");
    let admin_id = systemprompt_identifiers::UserId::new(format!(
        "serve-admin-{}",
        uuid::Uuid::new_v4().simple()
    ));
    seed_user_row_with_roles(
        &pool,
        &admin_id,
        "serve-admin@example.invalid",
        &["admin".to_owned()],
    )
    .await
    .expect("seed profile administrator");
    let raw_pool = pool.pool_arc().expect("raw dedicated database pool");
    sqlx::query("UPDATE users SET name = 'testadmin' WHERE id = $1")
        .bind(admin_id.as_str())
        .execute(raw_pool.as_ref())
        .await
        .expect("bind seeded administrator to profile username");

    let reservation = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve API port");
    let port = reservation.local_addr().unwrap().port();
    assert_eq!(ProcessCleanup::check_port(port), Some(std::process::id()));
    let fixture = isolated_fixture(port);
    complete_fixture_web_paths(&fixture);
    drop(reservation);

    let mut token_command = cli(&fixture.profile_path, database.url());
    token_command.args(["admin", "session", "login", "--token-only", "--force-new"]);
    let token_output = bounded_output(token_command, "mint API session");
    assert!(
        token_output.status.success(),
        "login failed: {}",
        String::from_utf8_lossy(&token_output.stderr)
    );
    let token = String::from_utf8(token_output.stdout)
        .expect("token output is UTF-8")
        .trim()
        .to_owned();
    assert_eq!(token.split('.').count(), 3, "CLI returned a JWT");

    let stderr = tempfile::NamedTempFile::new().expect("create API stderr capture");
    let output_capture = stderr.reopen().expect("open API output capture");
    let child = cli(&fixture.profile_path, database.url())
        .args(["infra", "services", "serve", "--foreground"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            output_capture
                .try_clone()
                .expect("clone API output capture"),
        ))
        .stderr(Stdio::from(output_capture))
        .spawn()
        .expect("start API through the public CLI");
    let mut server = OwnedServer {
        child,
        stderr,
        cleanup_agent: None,
    };
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .pool_max_idle_per_host(0)
        .build()
        .expect("HTTP client");
    wait_until_ready(&client, &base, &mut server).await;
    wait_for_owned_listener(port, server.child.id()).await;
    assert_eq!(
        ProcessCleanup::check_port(port),
        Some(server.child.id()),
        "only the owned CLI child may hold the API port"
    );

    let unauthenticated_status = client
        .get(format!("{base}/api/v1/health/detail"))
        .send()
        .await
        .expect("unauthenticated detailed health request")
        .status();
    assert_eq!(unauthenticated_status, reqwest::StatusCode::UNAUTHORIZED);
    let detail = client
        .get(format!("{base}/api/v1/health/detail"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("authenticated detailed health request");
    assert!(detail.status().is_success(), "status={}", detail.status());
    let detail: serde_json::Value = detail.json().await.expect("detailed health JSON");
    assert_eq!(
        detail["checks"]["database"]["status"], "healthy",
        "{detail}"
    );

    wait_for_owned_listener(port, server.child.id()).await;
    let mut stop_command = cli(&fixture.profile_path, database.url());
    stop_command
        .env("RUST_LOG", "off")
        .args(["--json", "infra", "services", "stop", "--api"]);
    let stop_output = bounded_output(stop_command, "stop owned API service");
    assert!(
        stop_output.status.success(),
        "public API stop failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&stop_output.stdout),
        String::from_utf8_lossy(&stop_output.stderr)
    );
    let stop: serde_json::Value = serde_json::from_slice(&stop_output.stdout).unwrap_or_else(|e| {
        panic!(
            "API stop output is not JSON: {e}\n{}",
            String::from_utf8_lossy(&stop_output.stdout)
        )
    });
    assert_eq!(stop["title"], "Stop Services", "{stop}");
    let api_stopped = stop["sections"]
        .as_array()
        .expect("stop sections")
        .iter()
        .find(|section| section["heading"] == "api_stopped")
        .expect("api_stopped section");
    assert_eq!(api_stopped["content"], true, "{stop}");
    let status = wait_for_exit(&mut server).await;
    assert!(status.success(), "graceful API exit: {status}");
    assert!(
        ProcessCleanup::wait_for_port_free(port, 20, 50)
            .await
            .is_ok()
    );
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user_sessions WHERE user_id = $1")
            .bind(admin_id.as_str())
            .fetch_one(raw_pool.as_ref())
            .await
            .expect("session count")
            >= 1
    );

    drop(server);
    drop(raw_pool);
    drop(pool);
    database.drop_now().await;
}
#[tokio::test]
async fn cli_serve_starts_routes_and_stops_an_owned_agent() {
    let database = DisposableDb::installed("cli_serve_agent_vertical")
        .await
        .expect("dedicated installed database");
    let pool = database.pool().await.expect("dedicated database pool");
    let raw_pool = pool.pool_arc().expect("raw dedicated database pool");
    let admin_id = systemprompt_identifiers::UserId::new(format!(
        "serve-agent-admin-{}",
        uuid::Uuid::new_v4().simple()
    ));
    seed_user_row_with_roles(
        &pool,
        &admin_id,
        "serve-agent-admin@example.invalid",
        &["admin".to_owned()],
    )
    .await
    .expect("seed profile administrator");
    sqlx::query("UPDATE users SET name = 'testadmin' WHERE id = $1")
        .bind(admin_id.as_str())
        .execute(raw_pool.as_ref())
        .await
        .expect("bind seeded administrator to profile username");

    let api_reservation = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve API port");
    let agent_reservation = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("reserve an allowed agent port");
    let api_port = api_reservation.local_addr().unwrap().port();
    let agent_port = agent_reservation.local_addr().unwrap().port();
    assert_ne!(api_port, agent_port);
    assert_eq!(
        ProcessCleanup::check_port(api_port),
        Some(std::process::id())
    );
    assert_eq!(
        ProcessCleanup::check_port(agent_port),
        Some(std::process::id())
    );
    let agent_name = format!(
        "covagent_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    );
    let fixture = isolated_fixture(api_port);
    systemprompt_cli_integration_tests::full_bootstrap::enable_fixture_agent(&fixture, agent_port);
    isolate_fixture_agent(&fixture, &agent_name);
    drop(agent_reservation);
    drop(api_reservation);

    let mut token_command = cli(&fixture.profile_path, database.url());
    token_command.args(["admin", "session", "login", "--token-only", "--force-new"]);
    let token_output = bounded_output(token_command, "mint agent-lifecycle API session");
    assert!(
        token_output.status.success(),
        "login failed: {}",
        String::from_utf8_lossy(&token_output.stderr)
    );
    let token = String::from_utf8(token_output.stdout)
        .expect("token output is UTF-8")
        .trim()
        .to_owned();

    let stderr = tempfile::NamedTempFile::new().expect("create API stderr capture");
    let output_capture = stderr.reopen().expect("open API output capture");
    let child = cli(&fixture.profile_path, database.url())
        .args(["infra", "services", "serve", "--foreground"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            output_capture
                .try_clone()
                .expect("clone API output capture"),
        ))
        .stderr(Stdio::from(output_capture))
        .spawn()
        .expect("start API and enabled agent through the public CLI");
    let mut server = OwnedServer {
        child,
        stderr,
        cleanup_agent: Some(OwnedAgent {
            name: agent_name.clone(),
            port: agent_port,
            observed_pids: Vec::new(),
            log_file: fixture
                .system_dir
                .join(format!("logs/agent-{agent_name}.log")),
        }),
    };
    let base = format!("http://127.0.0.1:{api_port}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .pool_max_idle_per_host(0)
        .build()
        .expect("HTTP client");
    wait_until_ready(&client, &base, &mut server).await;
    wait_for_owned_listener(api_port, server.child.id()).await;
    assert_eq!(
        ProcessCleanup::check_port(api_port),
        Some(server.child.id())
    );

    let (status, pid, stored_port): (String, Option<i32>, i32) =
        sqlx::query_as("SELECT status, pid, port FROM services WHERE name = $1")
            .bind(&agent_name)
            .fetch_one(raw_pool.as_ref())
            .await
            .expect("running agent service row");
    assert_eq!(status, "running");
    assert_eq!(stored_port, i32::from(agent_port));
    let agent_pid = u32::try_from(pid.expect("running agent PID")).expect("positive agent PID");
    server
        .cleanup_agent
        .as_mut()
        .expect("agent cleanup")
        .observed_pids
        .push(agent_pid);
    assert!(
        systemprompt_loader::subprocess::live_pid_is_subprocess(
            agent_pid,
            "AGENT_NAME",
            &agent_name
        ),
        "database PID must identify the owned agent"
    );

    let mut status_command = cli(&fixture.profile_path, database.url());
    status_command.env("RUST_LOG", "off");
    status_command.args(["--json", "infra", "services", "status", "--health"]);
    let status_output = bounded_output(status_command, "query live service status");
    assert!(
        status_output.status.success(),
        "service status failed: {}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status_artifact: serde_json::Value = serde_json::from_slice(&status_output.stdout)
        .unwrap_or_else(|error| {
            panic!(
                "service status output is not one JSON document: {error}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&status_output.stdout),
                String::from_utf8_lossy(&status_output.stderr)
            )
        });
    let agent_row = status_artifact["items"]
        .as_array()
        .expect("service status rows")
        .iter()
        .find(|row| row["name"] == agent_name)
        .unwrap_or_else(|| panic!("missing {agent_name} status: {status_artifact}"));
    assert_eq!(agent_row["service_type"], "agent", "{status_artifact}");
    assert_eq!(agent_row["status"], "running", "{status_artifact}");
    assert_eq!(agent_row["pid"], agent_pid, "{status_artifact}");
    assert_eq!(agent_row["port"], agent_port, "{status_artifact}");
    assert_eq!(agent_row["health"], "OK", "{status_artifact}");

    let direct_card: serde_json::Value = client
        .get(format!(
            "http://127.0.0.1:{agent_port}/.well-known/agent-card.json"
        ))
        .send()
        .await
        .expect("direct agent card request")
        .error_for_status()
        .expect("healthy direct agent card")
        .json()
        .await
        .expect("direct agent card JSON");
    assert_eq!(direct_card["name"], agent_name, "{direct_card}");

    let context_id = systemprompt_identifiers::ContextId::generate();
    let routed_response = client
        .get(format!(
            "{base}/api/v1/agents/{agent_name}/.well-known/agent-card.json"
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "agent-card-fixture",
            "method": "message/send",
            "params": { "message": { "contextId": context_id.as_str() } }
        }))
        .send()
        .await
        .expect("routed agent card request");
    let routed_status = routed_response.status();
    let routed_body = routed_response
        .bytes()
        .await
        .expect("routed agent card body");
    assert!(
        routed_status.is_success(),
        "API agent-card proxy returned {routed_status}: {}",
        String::from_utf8_lossy(&routed_body)
    );
    let routed_card: serde_json::Value = serde_json::from_slice(&routed_body)
        .unwrap_or_else(|error| panic!("routed agent card is not JSON: {error}: {routed_body:?}"));
    assert_eq!(routed_card, direct_card);

    wait_for_owned_listener(api_port, server.child.id()).await;
    assert_eq!(
        ProcessCleanup::check_port(api_port),
        Some(server.child.id()),
        "restart may only target the owned API process"
    );
    let replacement_stderr =
        tempfile::NamedTempFile::new().expect("create replacement API stderr capture");
    let replacement_output = replacement_stderr
        .reopen()
        .expect("open replacement API output capture");
    let replacement_child = cli(&fixture.profile_path, database.url())
        .args(["infra", "services", "restart", "api"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            replacement_output
                .try_clone()
                .expect("clone replacement API output capture"),
        ))
        .stderr(Stdio::from(replacement_output))
        .spawn()
        .expect("restart the owned API through the public CLI");
    let mut replacement = OwnedServer {
        child: replacement_child,
        stderr: replacement_stderr,
        cleanup_agent: Some(OwnedAgent {
            name: agent_name.clone(),
            port: agent_port,
            observed_pids: vec![agent_pid],
            log_file: fixture
                .system_dir
                .join(format!("logs/agent-{agent_name}.log")),
        }),
    };
    let _original_status = wait_for_exit(&mut server).await;
    server.cleanup_agent = None;
    drop(server);

    wait_until_ready(&client, &base, &mut replacement).await;
    let mut server = replacement;
    wait_for_owned_listener(api_port, server.child.id()).await;
    assert_eq!(
        ProcessCleanup::check_port(api_port),
        Some(server.child.id()),
        "restart command must become the replacement API process"
    );
    let (replacement_status, replacement_pid): (String, Option<i32>) =
        sqlx::query_as("SELECT status, pid FROM services WHERE name = $1")
            .bind(&agent_name)
            .fetch_one(raw_pool.as_ref())
            .await
            .expect("replacement agent service row");
    assert_eq!(replacement_status, "running");
    let replacement_agent_pid =
        u32::try_from(replacement_pid.expect("replacement agent PID")).expect("positive PID");
    server
        .cleanup_agent
        .as_mut()
        .expect("agent cleanup")
        .observed_pids
        .push(replacement_agent_pid);
    assert_ne!(replacement_agent_pid, agent_pid);
    assert!(
        systemprompt_loader::subprocess::live_pid_is_subprocess(
            replacement_agent_pid,
            "AGENT_NAME",
            &agent_name
        ),
        "replacement database PID must identify the owned agent"
    );
    assert!(
        !ProcessCleanup::process_exists(agent_pid)
            || systemprompt_loader::subprocess::is_zombie(agent_pid),
        "restart must terminate the original agent child"
    );

    let database_password = url::Url::parse(database.url())
        .expect("private database URL")
        .password()
        .expect("private database password")
        .to_owned();
    let sanitize_command_output = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .replace(database.url(), "postgres://<REDACTED>")
            .replace(&database_password, "<REDACTED>")
    };
    let authored_config_path = fixture.services_dir.join("config/config.yaml");
    let authored_config_before =
        std::fs::read(&authored_config_path).expect("read agent config before runtime stop");
    let mut stop_agent_command = cli(&fixture.profile_path, database.url());
    stop_agent_command.args(["--json", "infra", "services", "stop", "agent", &agent_name]);
    let stop_agent_output = bounded_output(stop_agent_command, "stop owned agent by name");
    assert!(
        stop_agent_output.status.success(),
        "owned agent stop failed; stdout={}; stderr={}",
        sanitize_command_output(&stop_agent_output.stdout),
        sanitize_command_output(&stop_agent_output.stderr)
    );
    let stop_agent: serde_json::Value = serde_json::from_slice(&stop_agent_output.stdout)
        .unwrap_or_else(|error| {
            panic!(
                "agent stop output is not one JSON artifact: {error}: {}",
                sanitize_command_output(&stop_agent_output.stdout)
            )
        });
    assert_eq!(stop_agent["title"], "Stop Agent", "{stop_agent}");
    let stop_field = |heading: &str| {
        stop_agent["sections"]
            .as_array()
            .expect("agent stop sections")
            .iter()
            .find(|section| section["heading"] == heading)
            .unwrap_or_else(|| panic!("missing {heading}: {stop_agent}"))["content"]
            .clone()
    };
    assert_eq!(stop_field("service_type"), "agent");
    assert_eq!(stop_field("service_name"), agent_name.as_str());
    assert_eq!(stop_field("stopped"), true);
    assert_eq!(
        stop_field("message"),
        format!("Agent {agent_name} stopped successfully")
    );
    ProcessCleanup::wait_for_port_free(agent_port, 40, 50)
        .await
        .expect("public stop releases the owned agent listener");
    assert!(
        !ProcessCleanup::process_exists(replacement_agent_pid)
            || systemprompt_loader::subprocess::is_zombie(replacement_agent_pid),
        "public stop must reap the replacement agent process"
    );
    let remaining_service_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM services WHERE name = $1")
            .bind(&agent_name)
            .fetch_one(raw_pool.as_ref())
            .await
            .expect("read service state after named stop");
    assert_eq!(remaining_service_rows, 0);
    assert_eq!(
        std::fs::read(&authored_config_path).expect("read agent config after runtime stop"),
        authored_config_before,
        "runtime stop must preserve the authored agent definition"
    );

    wait_for_owned_listener(api_port, server.child.id()).await;
    assert_eq!(
        ProcessCleanup::check_port(api_port),
        Some(server.child.id()),
        "recovery restart may only target the owned API process"
    );
    let recovery_stderr =
        tempfile::NamedTempFile::new().expect("create recovery API stderr capture");
    let recovery_output = recovery_stderr
        .reopen()
        .expect("open recovery API output capture");
    let recovery_child = cli(&fixture.profile_path, database.url())
        .args(["infra", "services", "restart", "api"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            recovery_output
                .try_clone()
                .expect("clone recovery API output capture"),
        ))
        .stderr(Stdio::from(recovery_output))
        .spawn()
        .expect("restart the API after the public named stop");
    let mut recovery = OwnedServer {
        child: recovery_child,
        stderr: recovery_stderr,
        cleanup_agent: Some(OwnedAgent {
            name: agent_name.clone(),
            port: agent_port,
            observed_pids: vec![replacement_agent_pid],
            log_file: fixture
                .system_dir
                .join(format!("logs/agent-{agent_name}.log")),
        }),
    };
    let _stopped_api_status = wait_for_exit(&mut server).await;
    server.cleanup_agent = None;
    drop(server);

    wait_until_ready(&client, &base, &mut recovery).await;
    let mut server = recovery;
    wait_for_owned_listener(api_port, server.child.id()).await;
    assert_eq!(
        ProcessCleanup::check_port(api_port),
        Some(server.child.id()),
        "recovery restart must own the API port"
    );
    let (restarted_status, restarted_pid): (String, Option<i32>) =
        sqlx::query_as("SELECT status, pid FROM services WHERE name = $1")
            .bind(&agent_name)
            .fetch_one(raw_pool.as_ref())
            .await
            .expect("agent service row after API recovery");
    assert_eq!(restarted_status, "running");
    let restarted_after_stop_pid =
        u32::try_from(restarted_pid.expect("recovered agent PID")).expect("positive PID");
    assert_ne!(restarted_after_stop_pid, replacement_agent_pid);
    assert!(
        systemprompt_loader::subprocess::live_pid_is_subprocess(
            restarted_after_stop_pid,
            "AGENT_NAME",
            &agent_name
        ),
        "recovery API must register the new owned agent process"
    );
    server
        .cleanup_agent
        .as_mut()
        .expect("agent cleanup")
        .observed_pids
        .push(restarted_after_stop_pid);
    wait_for_owned_listener(agent_port, restarted_after_stop_pid).await;

    kill(Pid::from_raw(server.child.id() as i32), Signal::SIGTERM)
        .expect("signal owned API parent");
    let status = wait_for_exit(&mut server).await;
    assert!(status.success(), "graceful API exit: {status}");
    assert!(
        ProcessCleanup::wait_for_port_free(api_port, 20, 50)
            .await
            .is_ok()
    );
    assert!(
        ProcessCleanup::wait_for_port_free(agent_port, 40, 50)
            .await
            .is_ok()
    );
    assert!(
        !ProcessCleanup::process_exists(restarted_after_stop_pid)
            || systemprompt_loader::subprocess::is_zombie(restarted_after_stop_pid),
        "API shutdown must terminate the agent restarted after the named stop"
    );

    drop(server);
    drop(raw_pool);
    drop(pool);
    database.drop_now().await;
}
