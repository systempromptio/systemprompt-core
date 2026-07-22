//! Orchestrator tests over a POPULATED registry: external servers scripted
//! via wiremock and internal servers registered through an extension manifest
//! whose binary is deliberately absent, driving the validation, start-failure,
//! reconcile, restart, and status paths that the empty-registry smoke tests
//! never reach.

use std::sync::Arc;
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_mcp::services::orchestrator::{McpEvent, McpOrchestrator};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, config_with_servers, default_tools_json, external_server_block,
    external_server_block_with_accessor, internal_server_block, mount_mcp_endpoint,
    register_internal_extension, write_services_config,
};

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

async fn orchestrator_with_config(blocks: &[String]) -> Option<McpOrchestrator> {
    let bootstrap = ensure_test_bootstrap();
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    write_services_config(bootstrap, &config_with_servers(blocks));
    let app_paths = Arc::new(AppPaths::from_profile(&profile_paths(bootstrap)).ok()?);
    let registry = RegistryService::new(fixture_user_id());
    McpOrchestrator::new(db, app_paths, registry).ok()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

#[tokio::test]
async fn validate_external_server_probe_succeeds_against_scripted_endpoint() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("valext");
    let Some(o) = orchestrator_with_config(&[external_server_block(&ExternalServerSpec {
        name: &name,
        endpoint: &format!("{}/mcp", mock.uri()),
        oauth_required: false,
        enabled: true,
    })])
    .await
    else {
        return;
    };

    o.validate_service(&name).await.expect("probe succeeds");
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
    let Some(o) = orchestrator_with_config(&[external_server_block(&ExternalServerSpec {
        name: &name,
        endpoint: "http://127.0.0.1:9/mcp",
        oauth_required: false,
        enabled: true,
    })])
    .await
    else {
        return;
    };

    o.validate_service(&name)
        .await
        .expect("a failed probe logs but does not error");
}

#[tokio::test]
async fn validate_external_server_with_accessor_skips_the_probe() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("valacc");
    let Some(o) = orchestrator_with_config(&[external_server_block_with_accessor(
        &name,
        &format!("{}/mcp", mock.uri()),
    )])
    .await
    else {
        return;
    };

    o.validate_service(&name).await.expect("accessor skip");
    let received = mock.received_requests().await.expect("requests recorded");
    assert!(
        received.is_empty(),
        "accessor-backed external servers are never probed"
    );
}

#[tokio::test]
async fn validate_internal_server_without_running_row_is_ok() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("valint");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65401)]).await else {
        return;
    };

    o.validate_service(&name)
        .await
        .expect("not-running internal service validates as a no-op");
}

#[tokio::test]
async fn validate_internal_running_server_probes_local_port() {
    let bootstrap = ensure_test_bootstrap();
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let port = mock.address().port();
    let name = unique("valrun");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, port)]).await else {
        return;
    };
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let db = fixture_db_pool(&url).await.expect("pool");
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let result = o.validate_service(&name).await;
    repo.delete_service(&name).await.unwrap();
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
    let bootstrap = ensure_test_bootstrap();
    let name = unique("startfail");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65402)]).await else {
        return;
    };
    let mut rx = o.subscribe_events();

    let err = o
        .start_services(Some(name.clone()))
        .await
        .expect_err("missing binary fails startup");
    assert!(err.to_string().contains(&name));
    assert!(err.to_string().contains("Failed to start 1 services"));

    let mut saw_requested = false;
    let mut saw_failed = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            McpEvent::ServiceStartRequested { service_name } if service_name == name => {
                saw_requested = true;
            },
            McpEvent::ServiceFailed { service_name, .. } if service_name == name => {
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
    let bootstrap = ensure_test_bootstrap();
    let name = unique("startnone");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65403)]).await else {
        return;
    };

    o.start_services(Some(unique("absent")))
        .await
        .expect("an unmatched name filter starts nothing");
}

#[tokio::test]
async fn reconcile_with_failing_internal_server_aggregates_the_failure() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("recfail");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65404)]).await else {
        return;
    };

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
    let Some(o) = orchestrator_with_config(&[external_server_block(&ExternalServerSpec {
        name: &name,
        endpoint: &format!("{}/mcp", mock.uri()),
        oauth_required: false,
        enabled: true,
    })])
    .await
    else {
        return;
    };

    let started = o.reconcile().await.expect("nothing to start");
    assert_eq!(started, 0, "external servers are excluded from reconcile");
}

#[tokio::test]
async fn restart_services_sync_missing_binary_fails_after_clean_stop() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("restart");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65405)]).await else {
        return;
    };

    let err = o
        .restart_services_sync(Some("all".to_owned()))
        .await
        .map(|()| String::new());
    assert!(
        err.is_ok(),
        "restart of 'all' over the DB running set is empty and succeeds"
    );

    let Ok(url) = fixture_database_url() else {
        return;
    };
    let db = fixture_db_pool(&url).await.expect("pool");
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port: 65405,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let result = o.restart_services_sync(None).await;
    repo.delete_service(&name).await.ok();
    let err = result.expect_err("restart start-phase fails on a missing binary");
    assert!(
        err.to_string().contains("Binary not found") || err.to_string().contains(&name),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn restart_services_publishes_restart_requested_event() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("restartreq");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65406)]).await else {
        return;
    };
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let db = fixture_db_pool(&url).await.expect("pool");
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port: 65406,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let mut rx = o.subscribe_events();
    let result = o.restart_services(None).await;
    repo.delete_service(&name).await.ok();
    result.expect("restart request publishes without touching processes");

    let mut saw_restart = false;
    while let Ok(event) = rx.try_recv() {
        if let McpEvent::ServiceRestartRequested { service_name, .. } = event
            && service_name == name
        {
            saw_restart = true;
        }
    }
    assert!(saw_restart, "ServiceRestartRequested published for {name}");
}

#[tokio::test]
async fn stop_services_named_internal_without_row_publishes_stopped() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("stopper");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65407)]).await else {
        return;
    };
    let mut rx = o.subscribe_events();

    o.stop_services(Some(name.clone()))
        .await
        .expect("stopping a not-running service is a clean no-op");

    let mut saw_stopped = false;
    while let Ok(event) = rx.try_recv() {
        if let McpEvent::ServiceStopped { service_name, .. } = event
            && service_name == name
        {
            saw_stopped = true;
        }
    }
    assert!(saw_stopped, "ServiceStopped published for {name}");
}

#[tokio::test]
async fn service_statuses_reports_external_endpoint_and_internal_port() {
    let bootstrap = ensure_test_bootstrap();
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let ext_name = unique("stext");
    let int_name = unique("stint");
    register_internal_extension(bootstrap, &int_name);
    let Some(o) = orchestrator_with_config(&[
        external_server_block(&ExternalServerSpec {
            name: &ext_name,
            endpoint: &format!("{}/mcp", mock.uri()),
            oauth_required: false,
            enabled: true,
        }),
        internal_server_block(&int_name, 65408),
    ])
    .await
    else {
        return;
    };

    let statuses = o.service_statuses().await.expect("statuses");
    let ext = statuses
        .iter()
        .find(|s| s.name == ext_name)
        .expect("external listed");
    assert_eq!(ext.port, 0);
    assert_eq!(
        ext.endpoint.as_deref(),
        Some(&*format!("{}/mcp", mock.uri()))
    );
    assert!(!ext.auth_required);

    let int = statuses
        .iter()
        .find(|s| s.name == int_name)
        .expect("internal listed");
    assert_eq!(int.port, 65408);
    assert!(int.endpoint.is_none());
    assert!(int.pid.is_none());
}

#[tokio::test]
async fn list_services_and_show_status_render_the_populated_registry() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let name = unique("display");
    let Some(o) = orchestrator_with_config(&[external_server_block(&ExternalServerSpec {
        name: &name,
        endpoint: &format!("{}/mcp", mock.uri()),
        oauth_required: false,
        enabled: true,
    })])
    .await
    else {
        return;
    };

    o.list_services().await.expect("list renders");
    o.show_status().await.expect("status renders");
}

#[tokio::test]
async fn reconcile_with_events_kills_running_row_and_reports_cleanup() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("reckill");
    register_internal_extension(bootstrap, &name);
    let Some(o) = orchestrator_with_config(&[internal_server_block(&name, 65407)]).await else {
        return;
    };
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let db = fixture_db_pool(&url).await.expect("pool");
    let repo = ServiceRepository::new(&db).unwrap();

    let disabled = unique("recgone");
    repo.create_service(CreateServiceInput {
        name: &disabled,
        module_name: "mcp",
        status: "running",
        port: 65408,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("MCP_SERVICE_ID", &name)
        .spawn()
        .expect("spawn sleep");
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port: 65407,
        binary_mtime: None,
    })
    .await
    .unwrap();
    repo.update_service_pid(&name, i32::try_from(child.id()).unwrap())
        .await
        .unwrap();

    let (tx, mut rx) = systemprompt_traits::startup_channel();
    let result = o.reconcile_with_events(Some(&tx)).await;
    drop(tx);

    let disabled_row = repo.find_service_by_name(&disabled).await.unwrap();
    repo.delete_service(&name).await.ok();

    let err = result.expect_err("missing binary still fails the start phase");
    assert!(err.to_string().contains(&name));
    assert!(disabled_row.is_none(), "disabled service row is pruned");
    assert!(!child.wait().expect("child reaped").success());

    let mut saw_cleanup = false;
    while let Ok(Some(event)) = rx.try_next() {
        if matches!(
            event,
            systemprompt_traits::StartupEvent::McpServiceCleanup { .. }
        ) {
            saw_cleanup = true;
        }
    }
    assert!(saw_cleanup, "reconcile reports cleanup over the event channel");
}

