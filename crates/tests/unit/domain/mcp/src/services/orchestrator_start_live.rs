//! The *successful* start path of the MCP orchestrator, which every other
//! orchestrator suite misses because their stub binaries never exist or never
//! listen: a spawnable stub that answers the MCP handshake on its configured
//! port lets `start_services` and `reconcile` run to completion, registering a
//! service row and publishing the started/completed events.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_database::ServiceRepository;
use systemprompt_mcp::services::orchestrator::{McpEvent, McpOrchestrator};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};

use crate::harness::{
    config_with_servers, internal_server_block, register_internal_extension, write_services_config,
};

// A minimal MCP endpoint: enough of the streamable-HTTP handshake for the
// startup health probe (initialize, initialized, tools/list) to succeed. The
// tool list must be non-empty — the probe reads an empty list as "service may
// require authentication" and never reports healthy.
const STUB_SERVER: &str = r#"import json, os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_DELETE(self):
        self.send_response(200)
        self.end_headers()

    def do_POST(self):
        length = int(self.headers.get('content-length', 0))
        body = json.loads(self.rfile.read(length) or b'{}')
        method = body.get('method', '')
        if method.startswith('notifications/'):
            self.send_response(202)
            self.end_headers()
            return
        if method == 'initialize':
            result = {
                'protocolVersion': '2025-03-26',
                'capabilities': {'tools': {}},
                'serverInfo': {'name': 'stub', 'version': '1.0.0'},
            }
        elif method == 'tools/list':
            result = {'tools': [{
                'name': 'echo',
                'description': 'echoes its input',
                'inputSchema': {'type': 'object', 'properties': {}},
            }]}
        else:
            result = {}
        payload = json.dumps(
            {'jsonrpc': '2.0', 'id': body.get('id', 0), 'result': result}
        ).encode()
        self.send_response(200)
        self.send_header('content-type', 'application/json')
        self.send_header('mcp-session-id', 'stub-session')
        self.send_header('content-length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

ThreadingHTTPServer(('127.0.0.1', int(os.environ['MCP_PORT'])), Handler).serve_forever()
"#;

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

// Installs `<name>-bin` in the bootstrap bin dir as a launcher for the stub
// server. The script is leaked deliberately: the spawned child outlives the
// helper's scope.
fn install_stub_binary(bootstrap: &TestBootstrap, name: &str) -> PathBuf {
    let dir = tempfile::tempdir().expect("stub dir");
    let script_path = dir.path().join("mcp_stub.py");
    std::fs::write(&script_path, STUB_SERVER).expect("write stub");

    let binary = bootstrap.bin_path.join(format!("{name}-bin"));
    write_executable(
        &binary,
        &format!("#!/bin/sh\nexec python3 {}\n", script_path.display()),
    );
    std::mem::forget(dir);
    binary
}

// Internal MCP servers are validated against the 5000-5999 range, so an
// ephemeral port would be rejected by config validation before any spawn.
fn free_port() -> u16 {
    systemprompt_test_fixtures::free_port_in_range(5000..6000)
        .expect("no free port in the internal MCP range 5000-5999")
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

fn profile_paths(bootstrap: &TestBootstrap) -> PathsConfig {
    PathsConfig {
        system: bootstrap.system_path.display().to_string(),
        services: bootstrap.services_path.display().to_string(),
        bin: bootstrap.bin_path.display().to_string(),
        web_path: None,
        storage: Some(bootstrap.storage_path.display().to_string()),
        geoip_database: None,
    }
}

struct LiveServer {
    orchestrator: McpOrchestrator,
    name: String,
    repo: ServiceRepository,
}

impl LiveServer {
    async fn teardown(&self) {
        let _ = self
            .orchestrator
            .stop_services(Some(self.name.clone()))
            .await;
        let _ = self.repo.delete_service(&self.name).await;
    }
}

async fn live_server(prefix: &str) -> Option<LiveServer> {
    let bootstrap = ensure_test_bootstrap();
    let name = unique(prefix);
    register_internal_extension(bootstrap, &name);
    install_stub_binary(bootstrap, &name);

    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    write_services_config(
        bootstrap,
        &config_with_servers(&[internal_server_block(&name, free_port())]),
    );

    let app_paths = Arc::new(
        AppPaths::from_profile(
            &profile_paths(bootstrap),
            systemprompt_models::PathResolution::Canonicalize,
        )
        .ok()?,
    );
    let repo = ServiceRepository::new(&db).ok()?;
    let orchestrator = McpOrchestrator::new(
        db,
        repo.clone(),
        app_paths,
        RegistryService::new(fixture_user_id()),
    )
    .ok()?;

    Some(LiveServer {
        orchestrator,
        name,
        repo,
    })
}

#[tokio::test]
async fn start_services_registers_a_listening_server_and_publishes_started() {
    let Some(live) = live_server("startlive").await else {
        return;
    };
    let mut rx = live.orchestrator.subscribe_events();

    let result = live
        .orchestrator
        .start_services(Some(live.name.clone()))
        .await;
    let row = live.repo.find_service_by_name(&live.name).await;
    live.teardown().await;

    result.expect("a listening stub starts cleanly");

    let row = row
        .expect("service lookup succeeds")
        .expect("a started server is registered");
    assert_eq!(row.status, "running");
    assert!(row.pid.is_some_and(|p| p > 0), "the pid is recorded");

    let mut saw_started = false;
    while let Ok(event) = rx.try_recv() {
        if let McpEvent::ServiceStarted {
            service_name,
            process_id,
            ..
        } = event
            && service_name == live.name
        {
            saw_started = true;
            assert!(process_id > 0, "ServiceStarted carries the real pid");
        }
    }
    assert!(saw_started, "a successful start publishes ServiceStarted");
}

#[tokio::test]
async fn reconcile_starts_a_listening_server_and_reports_completion() {
    let Some(live) = live_server("reclive").await else {
        return;
    };
    let mut rx = live.orchestrator.subscribe_events();

    let (tx, mut startup_rx) = systemprompt_traits::startup_channel();
    let result = live.orchestrator.reconcile_with_events(Some(&tx)).await;
    drop(tx);
    live.teardown().await;

    assert_eq!(result.expect("reconcile starts the stub"), 1);

    let mut saw_completed = false;
    while let Ok(event) = rx.try_recv() {
        if let McpEvent::ServiceStartCompleted {
            service_name,
            success,
            pid,
            error,
            ..
        } = event
            && service_name == live.name
        {
            saw_completed = true;
            assert!(success, "the start succeeded");
            assert!(pid.is_some_and(|p| p > 0), "the completion carries the pid");
            assert!(error.is_none());
        }
    }
    assert!(saw_completed, "reconcile publishes ServiceStartCompleted");

    let mut complete = None;
    while let Ok(event) = startup_rx.try_recv() {
        if let systemprompt_traits::StartupEvent::McpReconciliationComplete { running, required } =
            event
        {
            complete = Some((running, required));
        }
    }
    assert_eq!(
        complete,
        Some((1, 1)),
        "reconciliation reports one of one running"
    );
}

#[tokio::test]
async fn a_second_reconcile_kills_the_previous_process_and_starts_a_fresh_one() {
    let Some(live) = live_server("recidem").await else {
        return;
    };

    let first = live.orchestrator.reconcile().await;
    let first_pid = live
        .repo
        .find_service_by_name(&live.name)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.pid);

    let second = live.orchestrator.reconcile().await;
    let second_pid = live
        .repo
        .find_service_by_name(&live.name)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.pid);
    live.teardown().await;

    assert_eq!(first.expect("first reconcile starts"), 1);
    assert_eq!(second.expect("second reconcile restarts"), 1);

    let first_pid = first_pid.expect("first run registered a pid");
    let second_pid = second_pid.expect("second run registered a pid");
    assert_ne!(
        first_pid, second_pid,
        "reconcile restarts from a clean slate rather than adopting the old process"
    );
}

#[tokio::test]
async fn stop_services_terminates_a_running_server_and_publishes_stopped() {
    let Some(live) = live_server("stoplive").await else {
        return;
    };

    live.orchestrator
        .start_services(Some(live.name.clone()))
        .await
        .expect("stub starts");
    let mut rx = live.orchestrator.subscribe_events();

    let result = live
        .orchestrator
        .stop_services(Some(live.name.clone()))
        .await;
    let row = live.repo.find_service_by_name(&live.name).await;
    let _ = live.repo.delete_service(&live.name).await;

    result.expect("a running server stops cleanly");
    assert_ne!(
        row.expect("lookup succeeds").map(|r| r.status).as_deref(),
        Some("running"),
        "the stopped server is no longer marked running"
    );

    let mut saw_stopped = false;
    while let Ok(event) = rx.try_recv() {
        if let McpEvent::ServiceStopped { service_name, .. } = event
            && service_name == live.name
        {
            saw_stopped = true;
        }
    }
    assert!(saw_stopped, "stopping publishes ServiceStopped");
}
