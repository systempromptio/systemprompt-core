use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{Database, DbPool, PoolConfig, ServiceRepository};
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::NodeRole;
use systemprompt_test_fixtures::{test_database_url, unique_instance};

async fn db_pool() -> DbPool {
    let cfg = PoolConfig {
        max_connections: 4,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
        statement_cache_capacity: 100,
    };
    Arc::new(
        Database::connect(&test_database_url(), None, &cfg)
            .await
            .expect("database"),
    )
}

async fn age_presence(db: &DbPool, instance: &InstanceId) {
    sqlx::query(
        "UPDATE instance_presence SET heartbeat_at = CURRENT_TIMESTAMP - make_interval(days => \
         1) WHERE instance_id = $1",
    )
    .bind(instance.as_str())
    .execute(&*db.write_pool())
    .await
    .expect("age presence");
}

async fn presence_role(db: &DbPool, instance: &InstanceId) -> Option<String> {
    sqlx::query_scalar("SELECT role FROM instance_presence WHERE instance_id = $1")
        .bind(instance.as_str())
        .fetch_optional(&*db.write_pool())
        .await
        .expect("read presence")
}

fn runner_count(counts: &[systemprompt_database::RoleCount]) -> i64 {
    counts
        .iter()
        .find(|c| c.role == NodeRole::Runner.as_str())
        .map_or(0, |c| c.instances)
}

#[tokio::test]
async fn touching_presence_upserts_the_role() {
    let db = db_pool().await;
    let instance = unique_instance();
    let repo = ServiceRepository::new(&db, instance.clone());
    repo.touch_presence(NodeRole::Worker).await.expect("touch");
    assert_eq!(
        presence_role(&db, &instance).await.as_deref(),
        Some("worker")
    );
    repo.touch_presence(NodeRole::Api)
        .await
        .expect("touch again");
    assert_eq!(presence_role(&db, &instance).await.as_deref(), Some("api"));
}

#[tokio::test]
async fn live_counts_include_fresh_replicas_and_skip_stale_ones() {
    let db = db_pool().await;
    let first = ServiceRepository::new(&db, unique_instance());
    let second = ServiceRepository::new(&db, unique_instance());
    let stale_id = unique_instance();
    let stale = ServiceRepository::new(&db, stale_id.clone());

    first.touch_presence(NodeRole::Runner).await.expect("touch");
    second
        .touch_presence(NodeRole::Runner)
        .await
        .expect("touch");
    stale.touch_presence(NodeRole::Runner).await.expect("touch");
    let with_stale = runner_count(&first.count_live_instances_by_role().await.expect("count"));
    assert!(with_stale >= 3, "{with_stale}");

    age_presence(&db, &stale_id).await;
    let counts = first.count_live_instances_by_role().await.expect("count");
    let roles: Vec<&str> = counts.iter().map(|c| c.role.as_str()).collect();
    let mut sorted = roles.clone();
    sorted.sort_unstable();
    assert_eq!(roles, sorted);
    assert!(runner_count(&counts) >= 2);
}

#[tokio::test]
async fn reachable_instances_need_fresh_presence_and_an_endpoint() {
    let db = db_pool().await;
    let instance = unique_instance();
    let repo = ServiceRepository::new(&db, instance.clone());
    repo.touch_presence(NodeRole::Worker).await.expect("touch");
    let listed = repo
        .list_reachable_instances(NodeRole::Worker, 60)
        .await
        .expect("list");
    assert!(!listed.contains(&instance), "no endpoint advertised yet");

    repo.advertise_endpoint("http://10.0.0.9:8080")
        .await
        .expect("advertise");
    let listed = repo
        .list_reachable_instances(NodeRole::Worker, 60)
        .await
        .expect("list");
    assert!(listed.contains(&instance));
    let other_role = repo
        .list_reachable_instances(NodeRole::Gateway, 60)
        .await
        .expect("list");
    assert!(!other_role.contains(&instance));

    age_presence(&db, &instance).await;
    let listed = repo
        .list_reachable_instances(NodeRole::Worker, 60)
        .await
        .expect("list");
    assert!(!listed.contains(&instance), "stale presence is unreachable");
}

#[tokio::test]
async fn stale_presence_is_deleted_and_fresh_kept() {
    let db = db_pool().await;
    let stale_id = unique_instance();
    let fresh_id = unique_instance();
    let stale = ServiceRepository::new(&db, stale_id.clone());
    let fresh = ServiceRepository::new(&db, fresh_id.clone());
    stale.touch_presence(NodeRole::Api).await.expect("touch");
    fresh.touch_presence(NodeRole::Api).await.expect("touch");
    age_presence(&db, &stale_id).await;

    let removed = fresh.delete_stale_presence(3600).await.expect("delete");
    assert!(removed >= 1, "{removed}");
    assert_eq!(presence_role(&db, &stale_id).await, None);
    assert_eq!(presence_role(&db, &fresh_id).await.as_deref(), Some("api"));
}
