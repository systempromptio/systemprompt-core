//! DB-backed tests for the coordination seams: replica liveness judged by the
//! instance claim and the `services` heartbeat, expiring single-owner leases
//! that a dead owner forfeits, and session-scoped advisory locks.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{
    CoordinationRepositories, CreateServiceInput, Database, DbPool, PoolConfig, ServiceModule,
    ServiceRepository, ServiceStatus,
};
use systemprompt_identifiers::{AdvisoryLockKey, InstanceId, LeaseKey, ServiceName};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_test_fixtures::test_database_url;
use systemprompt_traits::{AdvisoryLocks, CoordinationError, InstanceRegistry, LeaseRepository};

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn db_pool() -> DbPool {
    let url = test_database_url();
    let cfg = PoolConfig {
        max_connections: 4,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
        statement_cache_capacity: 100,
    };
    Arc::new(Database::connect(&url, None, &cfg).await.expect("database"))
}

struct Replica {
    services: ServiceRepository,
    coordination: CoordinationRepositories,
}

fn replica(db: &DbPool) -> Replica {
    let instance = InstanceId::new(unique("node"));
    Replica {
        services: ServiceRepository::new(db, instance.clone()),
        coordination: CoordinationRepositories::new(db, instance),
    }
}

async fn heartbeat_row(replica: &Replica) {
    replica
        .services
        .create_service(CreateServiceInput {
            name: &ServiceName::new(unique("svc")),
            module_name: ServiceModule::Mcp,
            status: ServiceStatus::Running,
            port: 5555,
            binary_mtime: None,
        })
        .await
        .expect("register service row");
}

async fn age_heartbeat(db: &DbPool, instance: &InstanceId) {
    sqlx::query(
        "UPDATE services SET heartbeat_at = CURRENT_TIMESTAMP - make_interval(days => 1) WHERE \
         instance_id = $1",
    )
    .bind(instance.as_str())
    .execute(&*db.write_pool())
    .await
    .expect("age heartbeat");
}

#[tokio::test]
async fn an_unknown_instance_is_not_alive() {
    let db = db_pool().await;
    let observer = replica(&db);
    let alive = observer
        .services
        .is_alive(&InstanceId::new(unique("ghost")))
        .await
        .expect("liveness");
    assert!(!alive);
}

#[tokio::test]
async fn a_held_instance_claim_is_alive_until_released() {
    let db = db_pool().await;
    let observer = replica(&db);
    let target = replica(&db);
    let instance = target.services.instance_id().clone();

    let claim = target.services.claim_instance().await.expect("claim");
    assert!(
        observer
            .services
            .is_alive(&instance)
            .await
            .expect("liveness")
    );

    claim.release().await;
    assert!(
        !observer
            .services
            .is_alive(&instance)
            .await
            .expect("liveness")
    );
}

#[tokio::test]
async fn a_fresh_heartbeat_is_alive_and_a_stale_one_is_not() {
    let db = db_pool().await;
    let observer = replica(&db);
    let target = replica(&db);
    let instance = target.services.instance_id().clone();

    heartbeat_row(&target).await;
    assert!(
        observer
            .services
            .is_alive(&instance)
            .await
            .expect("liveness")
    );

    age_heartbeat(&db, &instance).await;
    assert!(
        !observer
            .services
            .is_alive(&instance)
            .await
            .expect("liveness")
    );
}

#[tokio::test]
async fn a_live_owner_keeps_its_lease_and_renewing_extends_it() {
    let db = db_pool().await;
    let owner = replica(&db);
    let rival = replica(&db);
    let _claim = owner.services.claim_instance().await.expect("claim");
    let key = LeaseKey::new(unique("lease"));
    let ttl = Duration::from_secs(60);

    let first = owner
        .coordination
        .leases
        .claim(&key, ttl)
        .await
        .expect("claim lease")
        .expect("a free lease is granted");
    assert_eq!(&first.owner, owner.services.instance_id());

    let refused = rival
        .coordination
        .leases
        .claim(&key, ttl)
        .await
        .expect("claim");
    assert!(
        refused.is_none(),
        "a live owner's unexpired lease is not taken"
    );

    let renewed = owner
        .coordination
        .leases
        .claim(&key, Duration::from_secs(120))
        .await
        .expect("renew")
        .expect("the owner renews its own lease");
    assert_eq!(renewed.acquired_at, first.acquired_at);
    assert!(renewed.expires_at > first.expires_at);

    let holder = rival
        .coordination
        .leases
        .holder(&key)
        .await
        .expect("holder");
    assert_eq!(
        holder.map(|l| l.owner),
        Some(owner.services.instance_id().clone())
    );
}

#[tokio::test]
async fn a_released_lease_passes_to_the_next_claimant() {
    let db = db_pool().await;
    let owner = replica(&db);
    let rival = replica(&db);
    let _claim = owner.services.claim_instance().await.expect("claim");
    let key = LeaseKey::new(unique("lease"));
    let ttl = Duration::from_secs(60);

    owner
        .coordination
        .leases
        .claim(&key, ttl)
        .await
        .expect("claim");
    assert!(
        !rival
            .coordination
            .leases
            .release(&key)
            .await
            .expect("release")
    );
    assert!(
        owner
            .coordination
            .leases
            .release(&key)
            .await
            .expect("release")
    );

    let taken = rival
        .coordination
        .leases
        .claim(&key, ttl)
        .await
        .expect("claim")
        .expect("a released lease is free");
    assert_eq!(&taken.owner, rival.services.instance_id());
}

#[tokio::test]
async fn a_dead_owner_forfeits_an_unexpired_lease() {
    let db = db_pool().await;
    let owner = replica(&db);
    let rival = replica(&db);
    heartbeat_row(&owner).await;
    let key = LeaseKey::new(unique("lease"));
    let ttl = Duration::from_secs(3600);

    owner
        .coordination
        .leases
        .claim(&key, ttl)
        .await
        .expect("claim");
    assert!(
        rival
            .coordination
            .leases
            .claim(&key, ttl)
            .await
            .expect("claim")
            .is_none(),
        "a heartbeating owner keeps its lease"
    );

    age_heartbeat(&db, owner.services.instance_id()).await;
    let taken = rival
        .coordination
        .leases
        .claim(&key, ttl)
        .await
        .expect("claim")
        .expect("a dead owner's lease is forfeit");
    assert_eq!(&taken.owner, rival.services.instance_id());
}

#[tokio::test]
async fn an_expired_lease_is_free_even_while_its_owner_lives() {
    let db = db_pool().await;
    let owner = replica(&db);
    let rival = replica(&db);
    let _claim = owner.services.claim_instance().await.expect("claim");
    let key = LeaseKey::new(unique("lease"));

    owner
        .coordination
        .leases
        .claim(&key, Duration::from_secs(60))
        .await
        .expect("claim");
    sqlx::query(
        "UPDATE instance_leases SET expires_at = CURRENT_TIMESTAMP - make_interval(secs => 1) \
         WHERE lease_key = $1",
    )
    .bind(key.as_str())
    .execute(&*db.write_pool())
    .await
    .expect("expire lease");

    assert!(
        rival
            .coordination
            .leases
            .holder(&key)
            .await
            .expect("holder")
            .is_none()
    );
    let taken = rival
        .coordination
        .leases
        .claim(&key, Duration::from_secs(60))
        .await
        .expect("claim");
    assert!(taken.is_some());
}

#[tokio::test]
async fn a_ttl_outside_one_second_to_one_day_is_refused() {
    let db = db_pool().await;
    let owner = replica(&db);
    let key = LeaseKey::new(unique("lease"));
    for ttl in [Duration::from_millis(500), Duration::from_secs(86_401)] {
        let error = owner
            .coordination
            .leases
            .claim(&key, ttl)
            .await
            .unwrap_err();
        assert!(
            matches!(error, CoordinationError::InvalidTtl { .. }),
            "{error}"
        );
    }
}

#[tokio::test]
async fn an_advisory_lock_is_exclusive_until_released() {
    let db = db_pool().await;
    let first = replica(&db);
    let second = replica(&db);
    let key = AdvisoryLockKey::new(unique("lock"));

    let guard = first
        .coordination
        .locks
        .try_acquire(&key)
        .await
        .expect("lock")
        .expect("a free lock is granted");
    assert_eq!(guard.key(), &key);
    assert!(
        second
            .coordination
            .locks
            .acquire_within(&key, Duration::from_millis(300))
            .await
            .expect("lock")
            .is_none(),
        "a held lock is not granted to another session"
    );

    guard.release().await;
    let next = second
        .coordination
        .locks
        .try_acquire(&key)
        .await
        .expect("lock")
        .expect("a released lock is free");
    next.release().await;
}

#[tokio::test]
async fn dropping_an_advisory_lock_frees_it() {
    let db = db_pool().await;
    let first = replica(&db);
    let second = replica(&db);
    let key = AdvisoryLockKey::new(unique("lock"));

    let guard = first
        .coordination
        .locks
        .try_acquire(&key)
        .await
        .expect("lock")
        .expect("granted");
    drop(guard);

    let next = second
        .coordination
        .locks
        .acquire_within(&key, Duration::from_secs(5))
        .await
        .expect("lock")
        .expect("a dropped lock's session is closed, freeing it");
    next.release().await;
}

#[tokio::test]
async fn a_held_claim_reports_its_instance_and_session() {
    let db = db_pool().await;
    let target = replica(&db);
    let claim = target.services.claim_instance().await.expect("claim");

    assert_eq!(claim.instance_id(), target.services.instance_id());
    let rendered = format!("{claim:?}");
    assert!(rendered.contains("held: true"), "{rendered}");
    assert!(
        rendered.contains(target.services.instance_id().as_str()),
        "{rendered}"
    );
    claim.release().await;
}

#[tokio::test]
async fn an_unreachable_database_fails_the_claim_as_a_repository_error() {
    let dead = Arc::new(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_millis(250))
            .connect_lazy("postgres://nobody:nobody@127.0.0.1:1/does-not-exist")
            .expect("lazy pool"),
    );
    let db: DbPool = Arc::new(Database::from_pools(Arc::clone(&dead), Some(dead)));
    let repo = ServiceRepository::new(&db, InstanceId::new(unique("dead")));

    let err = repo.claim_instance().await.expect_err("no database");

    assert!(
        matches!(
            err,
            systemprompt_database::InstanceClaimError::Repository(_)
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn the_process_owner_is_the_live_replica_running_it_with_an_advertised_endpoint() {
    let db = db_pool().await;
    let owner = replica(&db);
    let api = replica(&db);
    let name = ServiceName::new(unique("mcp"));

    assert_eq!(
        api.services
            .find_process_owner(&name)
            .await
            .expect("lookup"),
        None,
        "no replica runs it yet"
    );

    owner
        .services
        .create_service(CreateServiceInput {
            name: &name,
            module_name: ServiceModule::Mcp,
            status: ServiceStatus::Running,
            port: 5556,
            binary_mtime: None,
        })
        .await
        .expect("register owner row");
    assert_eq!(
        api.services
            .find_process_owner(&name)
            .await
            .expect("lookup"),
        None,
        "an owner that advertises no endpoint is unreachable"
    );

    owner
        .services
        .advertise_endpoint("http://10.0.0.7:8080")
        .await
        .expect("advertise");
    owner
        .services
        .touch_presence(NodeRole::Worker)
        .await
        .expect("presence");
    assert_eq!(
        api.services
            .find_process_owner(&name)
            .await
            .expect("lookup"),
        None,
        "a worker that does not hold the process lease is not its owner"
    );

    owner
        .coordination
        .leases
        .claim(
            &LeaseKey::new(format!("mcp:{}", name.as_str())),
            Duration::from_secs(45),
        )
        .await
        .expect("claim")
        .expect("lease won");
    let found = api
        .services
        .find_process_owner(&name)
        .await
        .expect("lookup")
        .expect("owner found");
    assert_eq!(&found.instance_id, owner.services.instance_id());
    assert_eq!(found.advertise_url, "http://10.0.0.7:8080");
    assert_eq!(
        owner
            .services
            .find_process_owner(&name)
            .await
            .expect("lookup"),
        None,
        "a replica never forwards to itself"
    );

    age_heartbeat(&db, owner.services.instance_id()).await;
    assert_eq!(
        api.services
            .find_process_owner(&name)
            .await
            .expect("lookup"),
        None,
        "a stale owner is never chosen"
    );
}
