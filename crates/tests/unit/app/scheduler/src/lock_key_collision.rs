//! Two distinct lock holders sharing one advisory key is refused: the pure
//! check over derived keys, and the repository path over Postgres
//! `hashtext` with a pair of names whose hashes really collide.

use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{
    LockKeyCollision, SchedulerError, SchedulerRepository, find_lock_key_collision,
};
use systemprompt_test_fixtures::test_db_pool;

const COLLIDING_A: &str = "collide_job_171848";
const COLLIDING_B: &str = "collide_job_85702";

#[test]
fn distinct_names_with_one_key_are_a_collision_naming_both() {
    let keys = vec![
        (JobName::new("rollup"), 7),
        (JobName::new("cleanup"), 7),
        (JobName::new("other"), 9),
    ];
    assert_eq!(
        find_lock_key_collision(&keys),
        Some(LockKeyCollision {
            first: JobName::new("cleanup"),
            second: JobName::new("rollup"),
            key: 7,
        })
    );
}

#[test]
fn distinct_keys_are_not_a_collision() {
    let keys = vec![(JobName::new("rollup"), 7), (JobName::new("cleanup"), 8)];
    assert_eq!(find_lock_key_collision(&keys), None);
}

#[test]
fn two_holders_with_one_name_collide() {
    let keys = vec![
        (JobName::new("config_reconcile"), 7),
        (JobName::new("config_reconcile"), 7),
    ];
    assert_eq!(
        find_lock_key_collision(&keys),
        Some(LockKeyCollision {
            first: JobName::new("config_reconcile"),
            second: JobName::new("config_reconcile"),
            key: 7,
        })
    );
}

#[tokio::test]
async fn names_whose_hashtext_collides_are_refused_by_the_repository() {
    let pool = test_db_pool().await;
    let repo = SchedulerRepository::new(&pool);

    let err = repo
        .verify_lock_keys(&[
            JobName::new(COLLIDING_A),
            JobName::new("unrelated_job"),
            JobName::new(COLLIDING_B),
        ])
        .await
        .expect_err("a real hashtext collision must be refused");
    match err {
        SchedulerError::LockKeyCollision { first, second, .. } => {
            assert_eq!(first.as_str(), COLLIDING_A);
            assert_eq!(second.as_str(), COLLIDING_B);
        },
        other => panic!("expected a lock-key collision, got {other:?}"),
    }

    repo.verify_lock_keys(&[JobName::new(COLLIDING_A), JobName::new("unrelated_job")])
        .await
        .expect("names with distinct keys pass");
}
