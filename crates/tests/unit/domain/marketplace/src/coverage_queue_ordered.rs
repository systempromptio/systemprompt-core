//! Two transactions that grant over the same two owners in opposite order
//! queue those owners' coverage keys from separate statements. The queue
//! takes its key locks when each transaction commits, sorted, so neither
//! waits on the other mid-transaction and no run ends in a deadlock.

use systemprompt_identifiers::UserId;
use systemprompt_marketplace::managed::{ManagedRepository, NewResource, ResourceKind, SourceSpec};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};

const ITERATIONS: usize = 50;
const DEADLOCK: &str = "40P01";

async fn owner_with_resource(
    pool: &systemprompt_database::DbPool,
    repository: &ManagedRepository,
    label: &str,
) -> (UserId, String) {
    let owner = UserId::new(format!("coverage-{label}-{}", uuid::Uuid::new_v4()));
    seed_user_row(pool, &owner, &format!("{owner}@coverage-queue.invalid"))
        .await
        .expect("owner");
    let source = repository
        .register_source(&owner, "coverage-queue", &SourceSpec::Managed)
        .await
        .expect("source");
    let resource = repository
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source,
                upstream_key: "coverage-skill".to_owned(),
                kind: ResourceKind::Skill,
                resource_key: "coverage-skill".to_owned(),
            },
        )
        .await
        .expect("resource");
    (owner, resource.to_string())
}

async fn grant(
    tx: &mut sqlx::PgConnection,
    owner: &UserId,
    resource: &str,
    consumer: &UserId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO managed_consumer_grants (owner_id, resource_id, consumer_id) VALUES ($1, $2, $3)",
    )
    .bind(owner.as_str())
    .bind(resource)
    .bind(consumer.as_str())
    .execute(tx)
    .await
    .map(|_| ())
}

fn is_deadlock(result: &Result<(), sqlx::Error>) -> bool {
    matches!(result, Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some(DEADLOCK))
}

#[tokio::test]
async fn opposite_order_grants_over_shared_owners_never_deadlock() {
    let db = DisposableDb::with_schema("coverage_queue_ordered").await;
    let pool = db.test_pool().await;
    let raw = pool.write_pool();
    let repository = ManagedRepository::new(&pool);
    let (first, first_resource) = owner_with_resource(&pool, &repository, "a").await;
    let (second, second_resource) = owner_with_resource(&pool, &repository, "b").await;

    let mut deadlocks = 0_usize;
    for iteration in 0..ITERATIONS {
        sqlx::query(
            "DELETE FROM projection_backlog WHERE projection = 'managed_installation_coverage' \
             AND key = ANY($1)",
        )
        .bind(vec![first.to_string(), second.to_string()])
        .execute(raw.as_ref())
        .await
        .expect("clear the queued owners");
        let left = UserId::new(format!(
            "coverage-left-{iteration}-{}",
            uuid::Uuid::new_v4()
        ));
        let right = UserId::new(format!(
            "coverage-right-{iteration}-{}",
            uuid::Uuid::new_v4()
        ));
        for consumer in [&left, &right] {
            seed_user_row(
                &pool,
                consumer,
                &format!("{consumer}@coverage-queue.invalid"),
            )
            .await
            .expect("consumer");
        }

        let mut one = raw.begin().await.expect("begin one");
        let mut two = raw.begin().await.expect("begin two");
        grant(&mut one, &first, &first_resource, &left)
            .await
            .expect("one grants over the first owner");
        grant(&mut two, &second, &second_resource, &right)
            .await
            .expect("two grants over the second owner");
        let (one_second, two_first) = tokio::join!(
            grant(&mut one, &second, &second_resource, &left),
            grant(&mut two, &first, &first_resource, &right),
        );
        if is_deadlock(&one_second) || is_deadlock(&two_first) {
            deadlocks += 1;
            continue;
        }
        one_second.expect("one grants over the second owner");
        two_first.expect("two grants over the first owner");
        let (one_done, two_done) = tokio::join!(one.commit(), two.commit());
        one_done.expect("commit one");
        two_done.expect("commit two");

        let queued: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_backlog \
             WHERE projection = 'managed_installation_coverage' AND key = ANY($1)",
        )
        .bind(vec![first.to_string(), second.to_string()])
        .fetch_one(raw.as_ref())
        .await
        .expect("queued owners");
        assert_eq!(queued, 2, "both committed publishes queue both owners");
    }
    assert_eq!(deadlocks, 0, "{deadlocks} of {ITERATIONS} runs deadlocked");
}
