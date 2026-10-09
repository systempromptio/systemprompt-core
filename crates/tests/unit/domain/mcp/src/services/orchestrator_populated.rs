//! Orchestrator tests over a POPULATED registry: external servers scripted
//! via wiremock and internal servers registered through an extension manifest
//! whose binary is deliberately absent, driving the validation, start-failure,
//! reconcile, restart, and status paths that the empty-registry smoke tests
//! never reach.

use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess;
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_mcp::McpDomainError;
use systemprompt_mcp::services::orchestrator::{McpEvent, McpOrchestrator};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_test_fixtures::{TestBootstrap, fixture_user_id, test_db_pool};
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, bootstrap_with_services, config_with_servers, default_tools_json,
    external_server_block, external_server_block_with_accessor, internal_server_block,
    mount_mcp_endpoint, register_internal_extension,
};
use systemprompt_test_fixtures::unique_instance;

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

async fn orchestrator_with_config(blocks: &[String], internal: &[&str]) -> McpOrchestrator {
    orchestrator_and_repo(blocks, internal).await.0
}

async fn orchestrator_and_repo(
    blocks: &[String],
    internal: &[&str],
) -> (McpOrchestrator, ServiceRepository) {
    let bootstrap = bootstrap_with_services(&config_with_servers(blocks));
    let db = test_db_pool().await;
    for name in internal {
        register_internal_extension(bootstrap, name);
    }
    let app_paths = Arc::new(
        AppPaths::from_profile(
            &profile_paths(bootstrap),
            systemprompt_manifest::PathResolution::Canonicalize,
            None,
        )
        .expect("application paths"),
    );
    let registry = RegistryService::new(fixture_user_id());
    let service_repo = ServiceRepository::new(&db, unique_instance());
    let orchestrator =
        McpOrchestrator::new(service_repo.clone(), app_paths, registry).expect("MCP orchestrator");
    (orchestrator, service_repo)
}

// Internal MCP servers are validated against the 5000-5999 range, so a port
// outside it is rejected by services-config validation before any probe runs.
fn free_port() -> u16 {
    systemprompt_test_fixtures::free_port_in_range(5000..6000)
        .expect("no free port in the internal MCP range 5000-5999")
}

const LISTENER: &str = "import os, socket, time\n\
s = socket.socket()\n\
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\n\
s.bind(('127.0.0.1', int(os.environ['MCP_PORT'])))\n\
s.listen(8)\n\
time.sleep(30)\n";

async fn await_listening(port: u16) {
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("stand-in process never listened on port {port}");
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

#[tokio::test]
async fn validate_external_server_probe_succeeds_against_scripted_endpoint() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("valext");
    let o = orchestrator_with_config(
        &[external_server_block(&ExternalServerSpec {
            name: &name,
            endpoint: &format!("{}/mcp", mock.uri()),
            oauth_required: false,
            enabled: true,
        })],
        &[],
    )
    .await;

    o.validate_service(&ServiceName::new(name.as_str()))
        .await
        .expect("probe succeeds");
    let received = mock.received_requests().await.expect("requests recorded");
    assert!(
        received
            .iter()
            .any(|r| String::from_utf8_lossy(&r.body).contains("initialize")),
        "the probe performs an MCP initialize handshake"
    );
}

#[tokio::test]
async fn validate_external_server_unreachable_endpoint_is_reported_not_fatal() {
    let name = unique("valdown");
    let o = orchestrator_with_config(
        &[external_server_block(&ExternalServerSpec {
            name: &name,
            endpoint: "http://127.0.0.1:9/mcp",
            oauth_required: false,
            enabled: true,
        })],
        &[],
    )
    .await;

    o.validate_service(&ServiceName::new(name.as_str()))
        .await
        .expect("a failed probe logs but does not error");
}

#[tokio::test]
async fn validate_external_server_with_accessor_skips_the_probe() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("valacc");
    let o = orchestrator_with_config(
        &[external_server_block_with_accessor(
            &name,
            &format!("{}/mcp", mock.uri()),
        )],
        &[],
    )
    .await;

    o.validate_service(&ServiceName::new(name.as_str()))
        .await
        .expect("accessor skip");
    let received = mock.received_requests().await.expect("requests recorded");
    assert!(
        received.is_empty(),
        "accessor-backed external servers are never probed"
    );
}

#[tokio::test]
async fn validate_internal_server_without_running_row_is_ok() {
    let port = free_port();
    let name = unique("valint");
    let o = orchestrator_with_config(&[internal_server_block(&name, port)], &[&name]).await;

    o.validate_service(&ServiceName::new(name.as_str()))
        .await
        .expect("not-running internal service validates as a no-op");
}

#[tokio::test]
async fn validate_internal_running_server_probes_local_port() {
    let listener = systemprompt_test_fixtures::bind_in_range(5000..6000)
        .expect("no free port in the internal MCP range 5000-5999");
    let port = listener.local_addr().expect("addr").port();
    let mock = MockServer::builder().listener(listener).start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("valrun");
    let name_id = ServiceName::new(name.as_str());
    let (o, repo) = orchestrator_and_repo(&[internal_server_block(&name, port)], &[&name]).await;
    repo.create_service(CreateServiceInput {
        name: &name_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let result = o.validate_service(&ServiceName::new(name.as_str())).await;
    repo.delete_service(&name_id).await.unwrap();
    result.expect("running internal service probes 127.0.0.1:<port>");

    let received = mock.received_requests().await.expect("requests recorded");
    assert!(
        received
            .iter()
            .any(|r| String::from_utf8_lossy(&r.body).contains("initialize")),
        "the local probe reaches the scripted MCP endpoint"
    );
}

#[tokio::test]
async fn start_services_named_with_missing_binary_fails_and_publishes_failure() {
    let port = free_port();
    let name = unique("startfail");
    let o = orchestrator_with_config(&[internal_server_block(&name, port)], &[&name]).await;
    let mut rx = o.subscribe_events();

    let err = o
        .start_services(Some(ServiceName::new(name.as_str())))
        .await
        .expect_err("missing binary fails startup");
    match &err {
        McpDomainError::ServicesFailedToStart(failures) => {
            assert_eq!(failures.0.len(), 1, "{err}");
            assert_eq!(failures.0[0].service, name, "{err}");
        },
        other => panic!("expected ServicesFailedToStart, got {other:?}"),
    }

    let mut saw_requested = false;
    let mut saw_failed = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            McpEvent::ServiceStartRequested { service_name } if service_name.as_str() == name => {
                saw_requested = true;
            },
            McpEvent::ServiceFailed { service_name, .. } if service_name.as_str() == name => {
                saw_failed = true;
            },
            _ => {},
        }
    }
    assert!(saw_requested, "start publishes ServiceStartRequested");
    assert!(saw_failed, "failed start publishes ServiceFailed");
}

#[tokio::test]
async fn start_services_unknown_name_matches_nothing_and_succeeds() {
    let port = free_port();
    let name = unique("startnone");
    let o = orchestrator_with_config(&[internal_server_block(&name, port)], &[&name]).await;

    o.start_services(Some(ServiceName::new(unique("absent"))))
        .await
        .expect("an unmatched name filter starts nothing");
}

#[tokio::test]
async fn reconcile_with_failing_internal_server_aggregates_the_failure() {
    let port = free_port();
    let name = unique("recfail");
    let o = orchestrator_with_config(&[internal_server_block(&name, port)], &[&name]).await;

    let err = o.reconcile().await.expect_err("startup failure surfaces");
    assert!(
        err.to_string().contains("Failed to start 1 MCP service(s)"),
        "unexpected error: {err}"
    );
    assert!(err.to_string().contains(&name));
}

#[tokio::test]
async fn reconcile_external_only_registry_starts_nothing() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("recext");
    let o = orchestrator_with_config(
        &[external_server_block(&ExternalServerSpec {
            name: &name,
            endpoint: &format!("{}/mcp", mock.uri()),
            oauth_required: false,
            enabled: true,
        })],
        &[],
    )
    .await;

    let started = o.reconcile().await.expect("nothing to start");
    assert_eq!(started, 0, "external servers are excluded from reconcile");
}

#[tokio::test]
async fn restart_services_missing_binary_reports_a_failed_outcome() {
    let port = free_port();
    let name = unique("restart");
    let name_id = ServiceName::new(name.as_str());
    let (o, repo) = orchestrator_and_repo(&[internal_server_block(&name, port)], &[&name]).await;

    let outcomes = o
        .restart_services(None)
        .await
        .expect("listing the DB running set succeeds");
    assert!(
        outcomes.iter().all(|o| o.service_name.as_str() != name),
        "restart of 'all' covers only the DB running set, which lacks {name}"
    );

    repo.create_service(CreateServiceInput {
        name: &name_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let result = o
        .restart_services(Some(ServiceName::new(name.as_str())))
        .await;
    repo.delete_service(&name_id).await.ok();
    let outcomes = result.expect("target listing succeeds");
    assert_eq!(outcomes.len(), 1);
    let outcome = &outcomes[0];
    assert_eq!(outcome.service_name.as_str(), name);
    assert!(
        !outcome.is_restarted(),
        "a missing binary is not reported as restarted"
    );
    let err = outcome.result.as_ref().expect_err("start phase fails");
    assert!(
        err.to_string().contains("Binary not found") || err.to_string().contains(&name),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn stop_services_named_internal_without_row_publishes_stopped() {
    let port = free_port();
    let name = unique("stopper");
    let o = orchestrator_with_config(&[internal_server_block(&name, port)], &[&name]).await;
    let mut rx = o.subscribe_events();

    o.stop_services(Some(ServiceName::new(name.as_str())))
        .await
        .expect("stopping a not-running service is a clean no-op");

    let mut saw_stopped = false;
    while let Ok(event) = rx.try_recv() {
        if let McpEvent::ServiceStopped { service_name, .. } = event
            && service_name.as_str() == name
        {
            saw_stopped = true;
        }
    }
    assert!(saw_stopped, "ServiceStopped published for {name}");
}

#[tokio::test]
async fn service_statuses_reports_external_endpoint_and_internal_port() {
    let port = free_port();
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let ext_name = unique("stext");
    let int_name = unique("stint");
    let o = orchestrator_with_config(
        &[
            external_server_block(&ExternalServerSpec {
                name: &ext_name,
                endpoint: &format!("{}/mcp", mock.uri()),
                oauth_required: false,
                enabled: true,
            }),
            internal_server_block(&int_name, port),
        ],
        &[&int_name],
    )
    .await;

    let statuses = o.service_statuses().await.expect("statuses");
    let ext = statuses
        .iter()
        .find(|s| s.name.as_str() == ext_name)
        .expect("external listed");
    assert_eq!(ext.port, None);
    assert_eq!(
        ext.endpoint.as_deref(),
        Some(&*format!("{}/mcp", mock.uri()))
    );
    assert!(!ext.auth_required);

    let int = statuses
        .iter()
        .find(|s| s.name.as_str() == int_name)
        .expect("internal listed");
    assert_eq!(int.port, Some(port));
    assert!(int.endpoint.is_none());
    assert!(int.pid.is_none());
}

#[tokio::test]
async fn list_services_and_show_status_render_the_populated_registry() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("display");
    let o = orchestrator_with_config(
        &[external_server_block(&ExternalServerSpec {
            name: &name,
            endpoint: &format!("{}/mcp", mock.uri()),
            oauth_required: false,
            enabled: true,
        })],
        &[],
    )
    .await;

    o.list_services().await.expect("list renders");
    o.show_status().await.expect("status renders");
}

#[tokio::test]
async fn reconcile_with_events_kills_running_row_and_reports_cleanup() {
    let port = free_port();
    let name = unique("reckill");
    let name_id = ServiceName::new(name.as_str());
    let (o, repo) = orchestrator_and_repo(&[internal_server_block(&name, port)], &[&name]).await;

    let disabled = unique("recgone");
    let disabled_id = ServiceName::new(disabled.as_str());
    repo.create_service(CreateServiceInput {
        name: &disabled_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: 65408,
        binary_mtime: None,
    })
    .await
    .unwrap();

    // Why: reconcile demotes a "running" row whose port is dead to stopped and
    // then crashed, and only rows still marked running are signalled — so a
    // stand-in process that never listens is pruned on paper and left alive.
    let child = std::process::Command::new("python3")
        .arg("-c")
        .arg(LISTENER)
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("MCP_SERVICE_ID", &name)
        .env("MCP_PORT", port.to_string())
        .spawn()
        .expect("spawn port listener");
    await_listening(port).await;
    repo.create_service(CreateServiceInput {
        name: &name_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    repo.update_service_pid(&name_id, i32::try_from(child.id()).unwrap())
        .await
        .unwrap();

    let (tx, mut rx) = systemprompt_traits::startup_channel();
    let result = o.reconcile_with_events(Some(&tx)).await;
    drop(tx);

    let disabled_row = repo.find_service_by_name(&disabled_id).await.unwrap();
    repo.delete_service(&name_id).await.ok();

    let err = result.expect_err("missing binary still fails the start phase");
    assert!(err.to_string().contains(&name));
    assert!(disabled_row.is_none(), "disabled service row is pruned");
    assert!(!subprocess::is_running(child.id()).await);

    let mut saw_cleanup = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(
            event,
            systemprompt_traits::StartupEvent::McpServiceCleanup { .. }
        ) {
            saw_cleanup = true;
        }
    }
    assert!(
        saw_cleanup,
        "reconcile reports cleanup over the event channel"
    );
}
