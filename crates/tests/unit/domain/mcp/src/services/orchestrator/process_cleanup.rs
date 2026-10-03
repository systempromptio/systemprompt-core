//! Tests for the orchestrator's pre-start cleanup passes.
//!
//! `detect_and_handle_stale_binaries` is driven against a real file on disk
//! whose mtime is compared with the one recorded on the `services` row, and a
//! real child process that must be reaped when the binary is found to have been
//! rebuilt. `detect_and_handle_orphaned_processes` is driven against a socket
//! held by this test process, which exercises the registry lookup and the
//! identity guard that keeps the caller from signalling itself.

use std::net::TcpListener;
use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::orchestrator::process_cleanup::{
    detect_and_handle_orphaned_processes, detect_and_handle_stale_binaries,
};
use systemprompt_mcp::services::process::pid::get_process_name_by_pid;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::mcp::McpServerConfig;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_user_id, test_db_pool,
};

use crate::harness::internal_mcp_config;

const FIXTURE_PORT: u16 = 65500;

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

struct Fixture {
    bootstrap: &'static TestBootstrap,
    repo: ServiceRepository,
    database: DatabaseService,
}

async fn fixture() -> Fixture {
    let bootstrap = ensure_test_bootstrap();
    let db = test_db_pool().await;
    let app_paths = Arc::new(
        AppPaths::from_profile(
            &profile_paths(bootstrap),
            systemprompt_manifest::PathResolution::Canonicalize,
            None,
        )
        .expect("app paths"),
    );
    let repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    let database = DatabaseService::new(
        systemprompt_database::ServiceRepository::new(
            &db,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ),
        app_paths,
        RegistryService::new(fixture_user_id()),
    );
    Fixture {
        bootstrap,
        repo,
        database,
    }
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

fn write_binary(bootstrap: &TestBootstrap, name: &str) -> i64 {
    std::fs::create_dir_all(&bootstrap.bin_path).expect("create bin dir");
    let path = bootstrap
        .bin_path
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("write binary");

    let modified = path
        .metadata()
        .expect("metadata")
        .modified()
        .expect("mtime")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("epoch")
        .as_secs();
    i64::try_from(modified).expect("mtime fits i64")
}

struct RowSpec<'a> {
    name: &'a ServiceName,
    status: ServiceStatus,
    binary_mtime: Option<i64>,
    port: u16,
    pid: u32,
}

async fn seed_row(repo: &ServiceRepository, spec: &RowSpec<'_>) {
    repo.create_service(CreateServiceInput {
        name: spec.name,
        module_name: ServiceModule::Mcp,
        status: spec.status,
        port: spec.port,
        binary_mtime: spec.binary_mtime,
    })
    .await
    .expect("create service row");
    repo.update_service_pid(spec.name, i32::try_from(spec.pid).expect("pid fits i32"))
        .await
        .expect("set pid");
}

fn running_row(name: &ServiceName, binary_mtime: Option<i64>, pid: u32) -> RowSpec<'_> {
    RowSpec {
        name,
        status: ServiceStatus::Running,
        binary_mtime,
        port: FIXTURE_PORT,
        pid,
    }
}

async fn sweep_stale(config: &McpServerConfig, database: &DatabaseService) -> usize {
    detect_and_handle_stale_binaries(std::slice::from_ref(config), database)
        .await
        .expect("stale-binary sweep")
}

const MARKER_HELPER: &str = "services::orchestrator::process_cleanup::marker_helper";

#[test]
#[ignore = "re-executed as a child process by rebuilt_binary_kills_the_running_process"]
fn marker_helper() {
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(std::time::Duration::from_secs(30));
}

#[tokio::test]
async fn rebuilt_binary_kills_the_running_process_and_drops_the_row() {
    let fx = fixture().await;
    let name = unique("stalebin");
    let id = ServiceName::new(name.as_str());
    let current = write_binary(fx.bootstrap, &name);

    let mut marked = systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, &name);
    seed_row(
        &fx.repo,
        &running_row(&id, Some(current - 3600), marked.pid()),
    )
    .await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&id).await.expect("lookup");
    fx.repo.delete_service(&id).await.ok();

    assert_eq!(
        restarted, 1,
        "a rebuilt binary restarts exactly one service"
    );
    assert!(row.is_none(), "the stale service is unregistered");
    assert!(
        !marked.child.wait().expect("child reaped").success(),
        "the process running the old binary is terminated"
    );
}

#[tokio::test]
async fn unchanged_binary_leaves_the_service_registered() {
    let fx = fixture().await;
    let name = unique("freshbin");
    let id = ServiceName::new(name.as_str());
    let current = write_binary(fx.bootstrap, &name);
    seed_row(
        &fx.repo,
        &running_row(&id, Some(current), std::process::id()),
    )
    .await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&id).await.expect("lookup");
    fx.repo.delete_service(&id).await.ok();

    assert_eq!(restarted, 0);
    assert!(row.is_some(), "a matching mtime must not unregister");
}

#[tokio::test]
async fn service_without_a_recorded_mtime_is_never_stale() {
    let fx = fixture().await;
    let name = unique("nomtime");
    let id = ServiceName::new(name.as_str());
    write_binary(fx.bootstrap, &name);
    seed_row(&fx.repo, &running_row(&id, None, std::process::id())).await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&id).await.expect("lookup");
    fx.repo.delete_service(&id).await.ok();

    assert_eq!(restarted, 0);
    assert!(row.is_some());
}

#[tokio::test]
async fn unresolvable_binary_is_never_stale() {
    let fx = fixture().await;
    let name = unique("gonebin");
    let id = ServiceName::new(name.as_str());
    seed_row(&fx.repo, &running_row(&id, Some(1), std::process::id())).await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&id).await.expect("lookup");
    fx.repo.delete_service(&id).await.ok();

    assert_eq!(
        restarted, 0,
        "a service whose binary cannot be resolved is left alone"
    );
    assert!(row.is_some());
}

#[tokio::test]
async fn stopped_service_is_never_stale() {
    let fx = fixture().await;
    let name = unique("stopped");
    let id = ServiceName::new(name.as_str());
    let current = write_binary(fx.bootstrap, &name);
    seed_row(
        &fx.repo,
        &RowSpec {
            name: &id,
            status: ServiceStatus::Stopped,
            binary_mtime: Some(current - 3600),
            port: FIXTURE_PORT,
            pid: std::process::id(),
        },
    )
    .await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&id).await.expect("lookup");
    fx.repo.delete_service(&id).await.ok();

    assert_eq!(restarted, 0, "only running services are restarted");
    assert!(row.is_some());
}

#[tokio::test]
async fn empty_registry_and_unbound_ports_hold_no_orphans() {
    let fx = fixture().await;

    let none = detect_and_handle_orphaned_processes(&[], &fx.database)
        .await
        .expect("empty sweep");
    assert_eq!(none, 0);

    let free_port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };
    let config = internal_mcp_config(&unique("noorphan"), free_port);
    let swept = detect_and_handle_orphaned_processes(std::slice::from_ref(&config), &fx.database)
        .await
        .expect("free-port sweep");

    assert_eq!(swept, 0, "an unbound port holds no orphan");
}

#[tokio::test]
async fn port_holder_is_an_orphan_only_while_unregistered_and_is_never_signalled() {
    let fx = fixture().await;
    // skip-ok: `ps` is unavailable on this host, so the test process has no
    // readable name
    let Some(self_name) = get_process_name_by_pid(std::process::id()) else {
        return;
    };

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let config = internal_mcp_config(&self_name, port);
    let self_id = ServiceName::new(self_name.as_str());

    let orphaned =
        detect_and_handle_orphaned_processes(std::slice::from_ref(&config), &fx.database)
            .await
            .expect("unregistered sweep");

    seed_row(
        &fx.repo,
        &RowSpec {
            name: &self_id,
            status: ServiceStatus::Running,
            binary_mtime: None,
            port,
            pid: std::process::id(),
        },
    )
    .await;

    let registered =
        detect_and_handle_orphaned_processes(std::slice::from_ref(&config), &fx.database)
            .await
            .expect("registered sweep");

    fx.repo.delete_service(&self_id).await.ok();

    assert_eq!(
        orphaned, 1,
        "a port holder with no service row is reported as an orphan"
    );
    assert_eq!(
        registered, 0,
        "a port holder that owns a service row is not an orphan"
    );
    assert!(
        listener.local_addr().is_ok(),
        "the identity guard leaves the unmarked caller running"
    );
}
