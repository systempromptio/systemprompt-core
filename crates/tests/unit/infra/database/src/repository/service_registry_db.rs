//! DB-backed tests for the instance scoping of `ServiceRepository`.
//!
//! Two repositories over one database, each bound to a different replica id,
//! register the same service name and must never see, alter or reap each
//! other's rows. Only the heartbeat reaper crosses instances.

use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_identifiers::InstanceId;

use crate::services::db_helper::pool;

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn two_repos() -> Option<(ServiceRepository, ServiceRepository, sqlx::PgPool)> {
    let db = pool().await?;
    let pg = (*db.write_pool_arc().ok()?).clone();
    let a = ServiceRepository::new(&db, InstanceId::new(unique("node_a"))).ok()?;
    let b = ServiceRepository::new(&db, InstanceId::new(unique("node_b"))).ok()?;
    Some((a, b, pg))
}

async fn register(repo: &ServiceRepository, name: &str, pid: i32) {
    repo.create_service(CreateServiceInput {
        name,
        module_name: "mcp",
        status: "running",
        port: 5555,
        binary_mtime: None,
    })
    .await
    .expect("create service");
    repo.update_service_pid(name, pid).await.expect("set pid");
}

async fn age_heartbeat(pg: &sqlx::PgPool, instance_id: &InstanceId, secs: i64) {
    sqlx::query(
        "UPDATE services SET heartbeat_at = CURRENT_TIMESTAMP - make_interval(secs => $2) WHERE \
         instance_id = $1",
    )
    .bind(instance_id.as_str())
    .bind(secs as f64)
    .execute(pg)
    .await
    .expect("age heartbeat");
}

#[tokio::test]
async fn same_name_on_two_instances_is_two_rows() {
    let Some((a, b, _pg)) = two_repos().await else {
        return;
    };
    let name = unique("svc");
    register(&a, &name, 111).await;
    register(&b, &name, 222).await;

    let seen_by_a = a.find_service_by_name(&name).await.unwrap().unwrap();
    let seen_by_b = b.find_service_by_name(&name).await.unwrap().unwrap();
    assert_eq!(seen_by_a.pid, Some(111));
    assert_eq!(seen_by_b.pid, Some(222));
    assert_eq!(&seen_by_a.instance_id, a.instance_id());
    assert_eq!(&seen_by_b.instance_id, b.instance_id());

    let listed_by_a = a.list_mcp_services().await.unwrap();
    assert!(listed_by_a.iter().all(|row| &row.instance_id == a.instance_id()));
    assert_eq!(listed_by_a.iter().filter(|row| row.name == name).count(), 1);
}

#[tokio::test]
async fn delete_and_stale_cleanup_never_touch_other_instances() {
    let Some((a, b, _pg)) = two_repos().await else {
        return;
    };
    let name = unique("svc");
    register(&a, &name, 111).await;
    register(&b, &name, 222).await;

    a.delete_service(&name).await.unwrap();
    assert!(a.find_service_by_name(&name).await.unwrap().is_none());
    assert!(b.find_service_by_name(&name).await.unwrap().is_some());

    b.clear_service_pid(&name).await.unwrap();
    register(&a, &name, 333).await;
    a.cleanup_stale_entries().await.unwrap();
    assert!(a.find_service_by_name(&name).await.unwrap().is_some());
    assert!(
        b.find_service_by_name(&name).await.unwrap().is_some(),
        "a's stale sweep must not reap b's pid-less running row"
    );
    assert_eq!(b.cleanup_stale_entries().await.unwrap(), 1);
}

#[tokio::test]
async fn status_and_count_are_per_instance() {
    let Some((a, b, _pg)) = two_repos().await else {
        return;
    };
    let name = unique("svc");
    register(&a, &name, 111).await;
    register(&b, &name, 222).await;

    a.update_service_stopped(&name).await.unwrap();
    assert_eq!(a.count_running_services("mcp").await.unwrap(), 0);
    assert_eq!(b.count_running_services("mcp").await.unwrap(), 1);
    assert_eq!(
        b.find_service_by_name(&name).await.unwrap().unwrap().status,
        "running"
    );
}

#[tokio::test]
async fn heartbeat_touches_own_rows_and_reaper_crosses_instances() {
    let Some((a, b, pg)) = two_repos().await else {
        return;
    };
    let name = unique("svc");
    register(&a, &name, 111).await;
    register(&b, &name, 222).await;

    age_heartbeat(&pg, a.instance_id(), 600).await;
    age_heartbeat(&pg, b.instance_id(), 600).await;
    assert_eq!(a.touch_heartbeat().await.unwrap(), 1);

    let reaped = b.delete_dead_instances(90).await.unwrap();
    assert!(reaped >= 1);
    assert!(
        a.find_service_by_name(&name).await.unwrap().is_some(),
        "a heartbeated, so its row survives"
    );
    assert!(
        b.find_service_by_name(&name).await.unwrap().is_none(),
        "b stopped heartbeating, so any instance may reap it"
    );
}
