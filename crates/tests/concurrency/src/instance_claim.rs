//! Replica identity claims under concurrent boot.
//!
//! Each replica claims its instance id with a session advisory lock before
//! touching the `services` registry. These tests boot N replicas at once and
//! prove they reconcile without evicting each other, that a duplicate id is
//! refused while the first claim lives.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{
    CreateServiceInput, Database, DbPool, InstanceClaimError, PoolConfig, ServiceModule,
    ServiceRepository, ServiceStatus, UpsertServiceProcessInput,
};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_test_fixtures::test_database_url;

const REPLICAS: usize = 8;

async fn pool(max_connections: u32) -> DbPool {
    let cfg = PoolConfig {
        max_connections,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
        statement_cache_capacity: 100,
    };
    Arc::new(
        Database::connect(&test_database_url(), None, &cfg)
            .await
            .expect("connect to the test database"),
    )
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn boot_replica(db: DbPool, names: Vec<ServiceName>) -> Result<InstanceId, String> {
    let repo = ServiceRepository::new(&db, InstanceId::new(unique("replica")));
    let claim = repo.claim_instance().await.map_err(|e| e.to_string())?;

    for (offset, name) in names.iter().enumerate() {
        repo.create_service(CreateServiceInput {
            name,
            module_name: ServiceModule::Mcp,
            status: ServiceStatus::Starting,
            port: 6000,
            binary_mtime: None,
        })
        .await
        .map_err(|e| e.to_string())?;
        repo.upsert_service_process(UpsertServiceProcessInput {
            name,
            module_name: ServiceModule::Mcp,
            status: ServiceStatus::Running,
            pid: 10_000 + offset as i32,
            port: 6000 + offset as u16,
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    repo.cleanup_stale_entries()
        .await
        .map_err(|e| e.to_string())?;
    repo.touch_heartbeat().await.map_err(|e| e.to_string())?;
    repo.delete_dead_instances(90)
        .await
        .map_err(|e| e.to_string())?;

    let rows = repo.list_mcp_services().await.map_err(|e| e.to_string())?;
    let own: Vec<_> = rows
        .iter()
        .filter(|row| row.instance_id == *repo.instance_id())
        .collect();
    if own.len() != names.len() || rows.len() != own.len() {
        return Err(format!(
            "replica {} sees {} rows, {} of them its own, expected {}",
            repo.instance_id(),
            rows.len(),
            own.len(),
            names.len()
        ));
    }
    if own.iter().any(|row| row.status != ServiceStatus::Running) {
        return Err(format!(
            "replica {} has a non-running row",
            repo.instance_id()
        ));
    }

    let id = claim.instance_id().clone();
    claim.release().await;
    Ok(id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn n_replicas_with_distinct_ids_claim_and_reconcile_without_interference() {
    let db = pool(32).await;
    let names: Vec<ServiceName> = (0..3)
        .map(|i| ServiceName::new(format!("mcp-{i}")))
        .collect();

    let handles: Vec<_> = (0..REPLICAS)
        .map(|_| tokio::spawn(boot_replica(Arc::clone(&db), names.clone())))
        .collect();

    let mut ids = Vec::new();
    let mut failures = Vec::new();
    for handle in handles {
        match handle.await.expect("replica task") {
            Ok(id) => ids.push(id),
            Err(e) => failures.push(e),
        }
    }

    assert!(failures.is_empty(), "replica failures: {failures:?}");
    assert_eq!(ids.len(), REPLICAS);
    ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    ids.dedup();
    assert_eq!(
        ids.len(),
        REPLICAS,
        "every replica holds a distinct identity"
    );
}

#[tokio::test]
async fn duplicate_instance_id_is_refused_while_claim_is_live() {
    let db = pool(4).await;
    let id = InstanceId::new(unique("dup"));
    let first = ServiceRepository::new(&db, id.clone());
    let second = ServiceRepository::new(&db, id.clone());

    let claim = first.claim_instance().await.expect("first claim");
    match second.claim_instance().await {
        Err(InstanceClaimError::Claimed { instance_id }) => assert_eq!(instance_id, id),
        other => panic!("expected Claimed, got {other:?}"),
    }

    claim.release().await;
    let reclaimed = second
        .claim_instance()
        .await
        .expect("claim succeeds after release");
    reclaimed.release().await;
}

#[tokio::test]
async fn dropped_claim_frees_the_identity() {
    let db = pool(4).await;
    let id = InstanceId::new(unique("drop"));
    let repo = ServiceRepository::new(&db, id);

    let claim = repo.claim_instance().await.expect("first claim");
    drop(claim);

    let mut reclaimed = None;
    for _ in 0..20 {
        match repo.claim_instance().await {
            Ok(claim) => {
                reclaimed = Some(claim);
                break;
            },
            Err(InstanceClaimError::Claimed { .. }) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
            Err(e) => panic!("unexpected claim error: {e}"),
        }
    }
    reclaimed
        .expect("a dropped claim's session closes and frees the id")
        .release()
        .await;
}
