//! Two `worker` replicas sharing one database split the supervised processes
//! by rendezvous hashing, a crashed worker's processes move to the survivor,
//! and the proxy's owner lookup resolves each process to its lease holder.

use std::collections::BTreeMap;


use systemprompt_api::services::server::lifecycle::process_ownership::{
    KeyAction, LEASE_TTL, OwnershipStep, ProcessKey, key_action, ownership_step,
};
use systemprompt_database::{
    CoordinationRepositories, CreateServiceInput, DbPool, ServiceModule, ServiceRepository,
    ServiceStatus,
};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::LeaseRepository;

struct Worker {
    id: InstanceId,
    services: ServiceRepository,
    coordination: CoordinationRepositories,
}

impl Worker {
    async fn boot(db: &DbPool, url: &str) -> Self {
        let id = InstanceId::new(format!("worker_{}", uuid::Uuid::new_v4().simple()));
        let worker = Self {
            services: ServiceRepository::new(db, id.clone()),
            coordination: CoordinationRepositories::new(db, id.clone()),
            id,
        };
        worker
            .services
            .advertise_endpoint(url)
            .await
            .expect("advertise");
        worker
            .services
            .touch_presence(NodeRole::Worker)
            .await
            .expect("presence");
        worker
    }

    async fn reachable_peers(&self, cohort: &[&InstanceId]) -> Vec<InstanceId> {
        self.services
            .list_reachable_instances(NodeRole::Worker, LEASE_TTL.as_secs() as i64)
            .await
            .expect("reachable workers")
            .into_iter()
            .filter(|id| cohort.contains(&id))
            .collect()
    }

    async fn tick(&self, keys: &[ProcessKey], workers: &[InstanceId], held: &mut Vec<ProcessKey>) {
        for key in keys {
            let holds = held.contains(key);
            let lease = key.lease_key();
            match key_action(&lease, &self.id, workers, workers, holds) {
                KeyAction::Claim | KeyAction::Renew => {
                    let claim = self
                        .coordination
                        .leases
                        .claim(&lease, LEASE_TTL)
                        .await
                        .expect("claim");
                    match ownership_step(holds, claim.as_ref(), &self.id) {
                        OwnershipStep::Acquired => {
                            self.services
                                .create_service(CreateServiceInput {
                                    name: key.name(),
                                    module_name: ServiceModule::Mcp,
                                    status: ServiceStatus::Running,
                                    port: 5600,
                                    binary_mtime: None,
                                })
                                .await
                                .expect("register running row");
                            held.push(key.clone());
                        },
                        OwnershipStep::Lost => held.retain(|k| k != key),
                        OwnershipStep::Renewed | OwnershipStep::Standby => {},
                    }
                },
                KeyAction::HandOver => {
                    self.coordination
                        .leases
                        .release(&lease)
                        .await
                        .expect("release");
                    held.retain(|k| k != key);
                },
                KeyAction::Skip => {},
            }
        }
    }
}

async fn crash(db: &DbPool, id: &InstanceId) {
    for table in ["services", "instance_endpoints", "instance_presence"] {
        sqlx::query(&format!(
            "UPDATE {table} SET heartbeat_at = CURRENT_TIMESTAMP - make_interval(days => 1) \
             WHERE instance_id = $1"
        ))
        .bind(id.as_str())
        .execute(&*db.write_pool())
        .await
        .expect("age heartbeat");
    }
}

#[tokio::test]
async fn two_workers_split_processes_and_the_survivor_takes_over_a_crashed_peer() {
    let db = test_db_pool().await;
    let a = Worker::boot(&db, "http://10.0.0.1:8080").await;
    let b = Worker::boot(&db, "http://10.0.0.2:8080").await;
    let api = ServiceRepository::new(
        &db,
        InstanceId::new(format!("api_{}", uuid::Uuid::new_v4().simple())),
    );
    let cohort = [&a.id, &b.id];
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let keys: Vec<ProcessKey> = (0..24)
        .map(|i| {
            ProcessKey::new(
                ServiceModule::Mcp,
                ServiceName::new(format!("svc{i}_{suffix}")),
            )
        })
        .collect();

    let workers = a.reachable_peers(&cohort).await;
    assert_eq!(workers.len(), 2, "both workers are reachable");
    let (mut held_a, mut held_b) = (Vec::new(), Vec::new());
    a.tick(&keys, &workers, &mut held_a).await;
    b.tick(&keys, &workers, &mut held_b).await;

    assert!(
        !held_a.is_empty() && !held_b.is_empty(),
        "the processes are split"
    );
    assert_eq!(
        held_a.len() + held_b.len(),
        keys.len(),
        "every process has one owner"
    );
    assert!(
        held_a.iter().all(|key| !held_b.contains(key)),
        "no process has two owners"
    );

    let expected: BTreeMap<&ServiceName, &InstanceId> = held_a
        .iter()
        .map(|key| (key.name(), &a.id))
        .chain(held_b.iter().map(|key| (key.name(), &b.id)))
        .collect();
    for (name, holder) in &expected {
        let owner = api
            .find_process_owner(name)
            .await
            .expect("lookup")
            .expect("owner found");
        assert_eq!(
            &&owner.instance_id, holder,
            "{name} resolves to its lease holder"
        );
    }

    crash(&db, &b.id).await;
    let survivors = a.reachable_peers(&cohort).await;
    assert_eq!(
        survivors,
        vec![a.id.clone()],
        "the crashed worker is unreachable"
    );
    a.tick(&keys, &survivors, &mut held_a).await;
    assert_eq!(held_a.len(), keys.len(), "the survivor owns every process");
    for key in &keys {
        let owner = api
            .find_process_owner(key.name())
            .await
            .expect("lookup")
            .expect("owner found");
        assert_eq!(
            owner.instance_id,
            a.id,
            "{} moved to the survivor",
            key.name()
        );
    }
}
