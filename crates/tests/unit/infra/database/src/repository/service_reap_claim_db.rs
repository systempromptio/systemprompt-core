//! `ServiceRepository::reap_stale` judges rows only under the instance claim:
//! while another process (here, a session tagged as a different machine) holds
//! the claim, a caller that does not hold it deletes nothing — not even a row
//! whose pid it cannot see — and reports `held_elsewhere`. Holding the claim
//! itself, or finding it free, the same pass reaps.

use sqlx::{Connection, PgConnection};
use systemprompt_database::{
    CreateServiceInput, INSTANCE_CLAIM_CLASS, ServiceModule, ServiceRepository, ServiceStatus,
};
use systemprompt_identifiers::{InstanceId, ServiceName, SpawnNonce};
use systemprompt_test_fixtures::{LivePids, test_database_url, test_db_pool, unique_name};

async fn running_row(repo: &ServiceRepository) -> ServiceName {
    let name = ServiceName::new(unique_name("claimed"));
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: 7100,
        binary_mtime: None,
    })
    .await
    .expect("create service");
    repo.update_service_pid(&name, 4242, &SpawnNonce::generate())
        .await
        .expect("set pid");
    name
}

async fn other_machine_holds(instance: &InstanceId) -> PgConnection {
    let mut session = PgConnection::connect(&test_database_url())
        .await
        .expect("holder session");
    sqlx::query(
        "SELECT set_config('application_name', 'systemprompt-claim:0ther0machine:1', false)",
    )
    .execute(&mut session)
    .await
    .expect("tag holder");
    sqlx::query("SELECT pg_advisory_lock($1, hashtext($2))")
        .bind(INSTANCE_CLAIM_CLASS)
        .bind(instance.as_str())
        .execute(&mut session)
        .await
        .expect("hold claim");
    session
}

async fn exists(repo: &ServiceRepository, name: &ServiceName) -> bool {
    repo.find_service_by_name(name)
        .await
        .expect("lookup")
        .is_some()
}

#[tokio::test]
async fn a_caller_without_the_claim_deletes_nothing_another_process_holds() {
    let db = test_db_pool().await;
    let instance = InstanceId::new(unique_name("held"));
    let repo = ServiceRepository::new(&db, instance.clone());
    let name = running_row(&repo).await;
    let holder = other_machine_holds(&instance).await;

    assert!(!repo.holds_claim().await.expect("claim check"));
    let reaped = repo.reap_stale(&LivePids::of([])).await.expect("reap");

    assert!(reaped.held_elsewhere);
    assert_eq!(reaped.total(), 0);
    assert!(exists(&repo, &name).await, "the holder's row survives");
    holder.close().await.expect("release holder");
}

#[tokio::test]
async fn the_claim_holder_reaps_and_a_free_claim_is_taken_for_the_pass() {
    let db = test_db_pool().await;
    let held = ServiceRepository::new(&db, InstanceId::new(unique_name("own")));
    let dead = running_row(&held).await;
    let claim = held.claim_instance().await.expect("claim");
    assert!(held.holds_claim().await.expect("claim check"));
    let reaped = held.reap_stale(&LivePids::of([])).await.expect("reap");
    assert!(!reaped.held_elsewhere);
    assert!(!exists(&held, &dead).await);
    claim.release().await;

    let free = ServiceRepository::new(&db, InstanceId::new(unique_name("free")));
    let stale = running_row(&free).await;
    let reaped = free.reap_stale(&LivePids::of([])).await.expect("reap");
    assert!(!reaped.held_elsewhere);
    assert!(!exists(&free, &stale).await);
    assert!(
        !free.holds_claim().await.expect("claim check"),
        "the pass releases its claim"
    );
}
