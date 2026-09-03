//! DB-backed tests for the `ServiceRepository` state transitions that the
//! scoping tests do not drive: the re-register upsert, the crash transition,
//! the stale sweep that reaps `error` rows, and the per-instance listings.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{CreateServiceInput, Database, DbPool, PoolConfig, ServiceRepository};
use systemprompt_identifiers::InstanceId;
use systemprompt_test_fixtures::fixture_database_url;

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn db_pool() -> DbPool {
    let url = fixture_database_url().expect("DATABASE_URL must be set");
    let cfg = PoolConfig {
        max_connections: 4,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
    };
    let db = Database::from_config_with_write("postgres", &url, None, &cfg)
        .await
        .expect("database");
    Arc::new(db)
}

async fn repo() -> (ServiceRepository, DbPool) {
    let db = db_pool().await;
    let repo = ServiceRepository::new(&db, InstanceId::new(unique("node"))).expect("repository");
    (repo, db)
}

async fn register(repo: &ServiceRepository, name: &str, module: &str, port: u16, pid: i32) {
    repo.create_service(CreateServiceInput {
        name,
        module_name: module,
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .expect("create service");
    repo.update_service_pid(name, pid).await.expect("set pid");
}

#[tokio::test]
async fn re_registering_the_same_name_updates_the_row_in_place() {
    let (repo, _db) = repo().await;
    let name = unique("svc");
    register(&repo, &name, "mcp", 5555, 111).await;

    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "agent",
        status: "starting",
        port: 6666,
        binary_mtime: Some(4242),
    })
    .await
    .expect("re-register");

    let row = repo
        .find_service_by_name(&name)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(row.module_name, "agent");
    assert_eq!(row.status, "starting");
    assert_eq!(row.port, 6666);
    assert_eq!(row.binary_mtime, Some(4242));
    assert_eq!(
        row.pid,
        Some(111),
        "the upsert must not clear a pid it does not write"
    );
    assert_eq!(repo.list_mcp_services().await.expect("list").len(), 0);
    assert_eq!(
        repo.list_all_agent_service_names().await.expect("names"),
        vec![name]
    );
}

#[tokio::test]
async fn crashing_clears_the_pid_and_the_stale_sweep_reaps_the_row() {
    let (repo, _db) = repo().await;
    let crashed = unique("svc_crashed");
    let healthy = unique("svc_healthy");
    register(&repo, &crashed, "mcp", 5555, 111).await;
    register(&repo, &healthy, "mcp", 5556, 222).await;

    repo.mark_service_crashed(&crashed).await.expect("crash");
    let row = repo
        .find_service_by_name(&crashed)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(row.status, "error");
    assert_eq!(row.pid, None, "a crashed service must not keep its pid");
    assert_eq!(repo.count_running_services("mcp").await.expect("count"), 1);

    assert_eq!(repo.cleanup_stale_entries().await.expect("sweep"), 1);
    assert!(
        repo.find_service_by_name(&crashed)
            .await
            .expect("find")
            .is_none()
    );
    assert!(
        repo.find_service_by_name(&healthy)
            .await
            .expect("find")
            .is_some(),
        "a running service with a live pid must survive the sweep"
    );
}

#[tokio::test]
async fn running_listings_are_status_filtered_and_name_ordered() {
    let (repo, _db) = repo().await;
    let mut names = vec![unique("svc_b"), unique("svc_a"), unique("svc_c")];
    for (index, name) in names.iter().enumerate() {
        register(
            &repo,
            name,
            "mcp",
            6000 + u16::try_from(index).expect("index fits"),
            300 + i32::try_from(index).expect("index fits"),
        )
        .await;
    }
    repo.update_service_status(&names[2], "starting")
        .await
        .expect("status");

    let running = repo
        .list_all_running_services()
        .await
        .expect("list running");
    names.truncate(2);
    names.sort();
    assert_eq!(
        running.iter().map(|r| r.name.clone()).collect::<Vec<_>>(),
        names,
        "only running rows, ordered by name"
    );
    assert_eq!(
        repo.list_running_services_with_pid()
            .await
            .expect("with pid")
            .len(),
        running.len()
    );
    assert_eq!(
        repo.list_services_by_type("mcp")
            .await
            .expect("by type")
            .len(),
        3,
        "the type listing is not status-filtered"
    );
}

#[tokio::test]
async fn dead_instance_reaper_honours_the_retention_boundary() {
    let (repo, db) = repo().await;
    let name = unique("svc");
    register(&repo, &name, "mcp", 5557, 111).await;

    // Why: the sweep is table-wide, so every retention this test passes sits
    // far outside any other test's rows — 400 days ages only its own row, and
    // both sweeps below straddle that age rather than a live window.
    let day_secs = 24 * 60 * 60;
    repo.delete_dead_instances(500 * day_secs)
        .await
        .expect("sweep");
    assert!(
        repo.find_service_by_name(&name)
            .await
            .expect("find")
            .is_some(),
        "a row heartbeated just now is inside the retention"
    );

    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query(
        "UPDATE services SET heartbeat_at = CURRENT_TIMESTAMP - make_interval(days => 400) WHERE \
         instance_id = $1",
    )
    .bind(repo.instance_id().as_str())
    .execute(&*pg)
    .await
    .expect("age heartbeat");

    assert!(
        repo.delete_dead_instances(399 * day_secs)
            .await
            .expect("sweep")
            >= 1
    );
    assert!(
        repo.find_service_by_name(&name)
            .await
            .expect("find")
            .is_none()
    );
}
