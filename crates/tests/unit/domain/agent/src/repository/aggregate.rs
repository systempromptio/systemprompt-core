use super::try_pool;
use systemprompt_agent::repository::A2ARepositories;

#[tokio::test]
async fn new_constructs_all_sub_repositories() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = A2ARepositories::new(&pool).expect("repos");
    // db_pool accessor returns the same underlying pool.
    let _ = repos.db_pool();
    // Each sub-repository is reachable.
    let _ = &repos.agent_services;
    let _ = &repos.tasks;
    let _ = &repos.execution_steps;
    let _ = &repos.push_notification_configs;
}

#[tokio::test]
async fn debug_format_contains_type_name() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = A2ARepositories::new(&pool).expect("repos");
    let dbg = format!("{repos:?}");
    assert!(dbg.contains("A2ARepositories"));
}
