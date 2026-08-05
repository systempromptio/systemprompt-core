//! Tests for the orchestrator's pre-start cleanup passes.
//!
//! `detect_and_handle_stale_binaries` is driven against a real file on disk
//! whose mtime is compared with the one recorded on the `services` row, and a
//! real child process that must be reaped when the binary is found to have been
//! rebuilt. `detect_and_handle_orphaned_processes` is driven against a socket
//! held by this test process, which exercises the registry lookup and the
//! identity guard that keeps the caller from signalling itself.

use std::net::TcpListener;
use std::process::Child;
use std::sync::Arc;

use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::process::pid::get_process_name_by_pid;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::test_api::{
    detect_and_handle_orphaned_processes, detect_and_handle_stale_binaries,
};
use systemprompt_models::AppPaths;
use systemprompt_models::mcp::McpServerConfig;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
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

async fn fixture() -> Option<Fixture> {
    let bootstrap = ensure_test_bootstrap();
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let app_paths = Arc::new(
        AppPaths::from_profile(
            &profile_paths(bootstrap),
            systemprompt_models::PathResolution::Canonicalize,
        )
        .ok()?,
    );
    let repo = ServiceRepository::new(&db).ok()?;
    let database = DatabaseService::new(
        systemprompt_database::ServiceRepository::new(&db).expect("service repository"),
        app_paths,
        RegistryService::new(fixture_user_id()),
    );
    Some(Fixture {
        bootstrap,
        repo,
        database,
    })
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
    name: &'a str,
    status: &'a str,
    binary_mtime: Option<i64>,
    port: u16,
    pid: u32,
}

async fn seed_row(repo: &ServiceRepository, spec: &RowSpec<'_>) {
    repo.create_service(CreateServiceInput {
        name: spec.name,
        module_name: "mcp",
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

fn running_row<'a>(name: &'a str, binary_mtime: Option<i64>, pid: u32) -> RowSpec<'a> {
    RowSpec {
        name,
        status: "running",
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

fn spawn_marked_child(service_name: &str) -> Child {
    let child = std::process::Command::new("sleep")
        .arg("30")
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("MCP_SERVICE_ID", service_name)
        .spawn()
        .expect("spawn sleep");
    await_environ(child.id(), service_name);
    child
}

// `spawn` returns between fork and exec; `/proc/<pid>/environ` still shows the
// parent's environment until exec completes, so an identity-verified signal
// would be skipped and the assertion under test would race the scheduler.
fn await_environ(pid: u32, marker: &str) {
    for _ in 0..500 {
        if let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ"))
            && String::from_utf8_lossy(&environ).contains(marker)
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("child {pid} never exposed {marker} in its environ");
}

#[tokio::test]
async fn rebuilt_binary_kills_the_running_process_and_drops_the_row() {
    let Some(fx) = fixture().await else { return };
    let name = unique("stalebin");
    let current = write_binary(fx.bootstrap, &name);

    let mut child = spawn_marked_child(&name);
    seed_row(
        &fx.repo,
        &running_row(&name, Some(current - 3600), child.id()),
    )
    .await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&name).await.expect("lookup");
    fx.repo.delete_service(&name).await.ok();

    assert_eq!(
        restarted, 1,
        "a rebuilt binary restarts exactly one service"
    );
    assert!(row.is_none(), "the stale service is unregistered");
    assert!(
        !child.wait().expect("child reaped").success(),
        "the process running the old binary is terminated"
    );
}

#[tokio::test]
async fn unchanged_binary_leaves_the_service_registered() {
    let Some(fx) = fixture().await else { return };
    let name = unique("freshbin");
    let current = write_binary(fx.bootstrap, &name);
    seed_row(
        &fx.repo,
        &running_row(&name, Some(current), std::process::id()),
    )
    .await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&name).await.expect("lookup");
    fx.repo.delete_service(&name).await.ok();

    assert_eq!(restarted, 0);
    assert!(row.is_some(), "a matching mtime must not unregister");
}

#[tokio::test]
async fn service_without_a_recorded_mtime_is_never_stale() {
    let Some(fx) = fixture().await else { return };
    let name = unique("nomtime");
    write_binary(fx.bootstrap, &name);
    seed_row(&fx.repo, &running_row(&name, None, std::process::id())).await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&name).await.expect("lookup");
    fx.repo.delete_service(&name).await.ok();

    assert_eq!(restarted, 0);
    assert!(row.is_some());
}

#[tokio::test]
async fn unresolvable_binary_is_never_stale() {
    let Some(fx) = fixture().await else { return };
    let name = unique("gonebin");
    seed_row(&fx.repo, &running_row(&name, Some(1), std::process::id())).await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&name).await.expect("lookup");
    fx.repo.delete_service(&name).await.ok();

    assert_eq!(
        restarted, 0,
        "a service whose binary cannot be resolved is left alone"
    );
    assert!(row.is_some());
}

#[tokio::test]
async fn stopped_service_is_never_stale() {
    let Some(fx) = fixture().await else { return };
    let name = unique("stopped");
    let current = write_binary(fx.bootstrap, &name);
    seed_row(
        &fx.repo,
        &RowSpec {
            name: &name,
            status: "stopped",
            binary_mtime: Some(current - 3600),
            port: FIXTURE_PORT,
            pid: std::process::id(),
        },
    )
    .await;

    let restarted = sweep_stale(&internal_mcp_config(&name, FIXTURE_PORT), &fx.database).await;

    let row = fx.repo.find_service_by_name(&name).await.expect("lookup");
    fx.repo.delete_service(&name).await.ok();

    assert_eq!(restarted, 0, "only running services are restarted");
    assert!(row.is_some());
}

#[tokio::test]
async fn empty_registry_and_unbound_ports_hold_no_orphans() {
    let Some(fx) = fixture().await else { return };

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
    let Some(fx) = fixture().await else { return };
    let Some(self_name) = get_process_name_by_pid(std::process::id()) else {
        return;
    };

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let config = internal_mcp_config(&self_name, port);

    let orphaned =
        detect_and_handle_orphaned_processes(std::slice::from_ref(&config), &fx.database)
            .await
            .expect("unregistered sweep");

    seed_row(
        &fx.repo,
        &RowSpec {
            name: &self_name,
            status: "running",
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

    fx.repo.delete_service(&self_name).await.ok();

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
