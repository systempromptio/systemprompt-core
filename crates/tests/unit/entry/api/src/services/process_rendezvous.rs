use std::collections::HashMap;

use systemprompt_api::services::server::lifecycle::process_ownership::{
    KeyAction, ProcessKey, key_action, with_self,
};
use systemprompt_api::services::server::lifecycle::rendezvous::{preferred_owner, score};
use systemprompt_database::ServiceModule;
use systemprompt_identifiers::{InstanceId, LeaseKey, ServiceName};

fn workers(n: usize) -> Vec<InstanceId> {
    (0..n)
        .map(|i| InstanceId::new(format!("worker-{i}")))
        .collect()
}

fn keys(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("mcp:service-{i}")).collect()
}

fn assignment(keys: &[String], workers: &[InstanceId]) -> HashMap<String, InstanceId> {
    keys.iter()
        .map(|key| {
            (
                key.clone(),
                preferred_owner(key, workers).expect("a worker").clone(),
            )
        })
        .collect()
}

#[test]
fn the_score_is_a_fixed_function_of_key_and_worker() {
    let worker = InstanceId::new("worker-a");
    assert_eq!(score("mcp:files", &worker), score("mcp:files", &worker));
    assert_ne!(score("mcp:files", &worker), score("mcp:search", &worker));
}

#[test]
fn the_score_never_changes_between_builds() {
    assert_eq!(
        score("mcp:files", &InstanceId::new("worker-a")),
        0xba11_e780_cc9b_ac25
    );
}

#[test]
fn every_worker_agrees_on_the_owner_whatever_order_it_lists_peers_in() {
    let forward = workers(5);
    let mut reversed = forward.clone();
    reversed.reverse();
    for key in keys(50) {
        assert_eq!(
            preferred_owner(&key, &forward),
            preferred_owner(&key, &reversed)
        );
    }
}

#[test]
fn no_workers_means_no_owner() {
    assert_eq!(preferred_owner("mcp:files", &[]), None);
}

#[test]
fn services_spread_across_workers_within_a_balanced_band() {
    let pool = workers(4);
    let all = keys(400);
    let mut per_worker: HashMap<InstanceId, usize> = HashMap::new();
    for owner in assignment(&all, &pool).into_values() {
        *per_worker.entry(owner).or_default() += 1;
    }
    assert_eq!(per_worker.len(), 4, "every worker owns something");
    for (worker, owned) in per_worker {
        assert!((60..=140).contains(&owned), "{worker} owns {owned} of 400");
    }
}

#[test]
fn a_joining_worker_takes_only_services_and_takes_them_from_others() {
    let before = workers(3);
    let after = workers(4);
    let joiner = after.last().expect("joiner").clone();
    let all = keys(300);
    let old = assignment(&all, &before);
    let new = assignment(&all, &after);
    let moved: Vec<&String> = all.iter().filter(|key| old[*key] != new[*key]).collect();
    assert!(
        moved.iter().all(|key| new[*key] == joiner),
        "only moves to the joiner"
    );
    assert!(
        (40..=110).contains(&moved.len()),
        "moved {} of 300",
        moved.len()
    );
}

#[test]
fn a_leaving_worker_moves_only_the_services_it_owned() {
    let before = workers(4);
    let leaver = before.last().expect("leaver").clone();
    let after = workers(3);
    let all = keys(300);
    let old = assignment(&all, &before);
    let new = assignment(&all, &after);
    for key in &all {
        if old[key] != leaver {
            assert_eq!(old[key], new[key], "{key} moved though its owner stayed");
        }
    }
}

#[test]
fn a_single_worker_owns_every_service() {
    let me = InstanceId::new("solo");
    let pool = with_self(Vec::new(), &me);
    for key in keys(20) {
        let lease = LeaseKey::new(key);
        assert_eq!(
            key_action(&lease, &me, &pool, &pool, false),
            KeyAction::Claim
        );
        assert_eq!(
            key_action(&lease, &me, &pool, &pool, true),
            KeyAction::Renew
        );
    }
}

#[test]
fn a_held_service_is_handed_over_only_once_its_preferred_owner_has_settled() {
    let pool = workers(2);
    let lease = keys(100)
        .into_iter()
        .map(LeaseKey::new)
        .find(|lease| preferred_owner(lease.as_str(), &pool) == Some(&pool[1]))
        .expect("a key preferring worker-1");
    let me = &pool[0];
    let only_me = vec![me.clone()];
    assert_eq!(
        key_action(&lease, me, &pool, &only_me, true),
        KeyAction::Renew
    );
    assert_eq!(
        key_action(&lease, me, &pool, &pool, true),
        KeyAction::HandOver
    );
    assert_eq!(key_action(&lease, me, &pool, &pool, false), KeyAction::Skip);
    assert_eq!(
        key_action(&lease, &pool[1], &pool, &only_me, false),
        KeyAction::Claim
    );
}

#[test]
fn with_self_adds_this_replica_once_and_orders_the_set() {
    let me = InstanceId::new("worker-b");
    let listed = vec![InstanceId::new("worker-c"), InstanceId::new("worker-a")];
    assert_eq!(
        with_self(listed.clone(), &me),
        vec![
            InstanceId::new("worker-a"),
            InstanceId::new("worker-b"),
            InstanceId::new("worker-c")
        ]
    );
    let mut with_me = listed;
    with_me.push(me.clone());
    assert_eq!(with_self(with_me, &me).len(), 3);
}

#[test]
fn a_process_lease_key_is_module_and_name() {
    let mcp = ProcessKey::new(ServiceModule::Mcp, ServiceName::new("files"));
    let agent = ProcessKey::new(ServiceModule::Agent, ServiceName::new("files"));
    assert_eq!(mcp.lease_key().as_str(), "mcp:files");
    assert_eq!(agent.lease_key().as_str(), "agent:files");
}
