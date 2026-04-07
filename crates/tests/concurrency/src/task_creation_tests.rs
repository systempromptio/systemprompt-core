use std::collections::HashSet;
use systemprompt_identifiers::TaskId;
use tokio::task::JoinSet;

#[tokio::test]
async fn concurrent_task_id_generation_produces_unique_ids() {
    let mut join_set = JoinSet::new();

    for _ in 0..100 {
        join_set.spawn(async { TaskId::generate() });
    }

    let mut ids = HashSet::new();
    while let Some(result) = join_set.join_next().await {
        let id = result.unwrap();
        assert!(
            ids.insert(id.as_str().to_string()),
            "Duplicate TaskId generated"
        );
    }

    assert_eq!(ids.len(), 100);
}

#[tokio::test]
async fn concurrent_context_id_generation_produces_unique_ids() {
    use systemprompt_identifiers::ContextId;

    let mut join_set = JoinSet::new();

    for _ in 0..100 {
        join_set.spawn(async { ContextId::generate() });
    }

    let mut ids = HashSet::new();
    while let Some(result) = join_set.join_next().await {
        let id = result.unwrap();
        assert!(
            ids.insert(id.as_str().to_string()),
            "Duplicate ContextId generated"
        );
    }

    assert_eq!(ids.len(), 100);
}

#[tokio::test]
async fn concurrent_task_default_construction() {
    use systemprompt_models::a2a::Task;

    let mut join_set = JoinSet::new();

    for _ in 0..50 {
        join_set.spawn(async { Task::default() });
    }

    let mut task_ids = HashSet::new();
    while let Some(result) = join_set.join_next().await {
        let task = result.unwrap();
        assert!(
            task_ids.insert(task.id.as_str().to_string()),
            "Duplicate Task ID from Task::default()"
        );
    }

    assert_eq!(task_ids.len(), 50);
}
