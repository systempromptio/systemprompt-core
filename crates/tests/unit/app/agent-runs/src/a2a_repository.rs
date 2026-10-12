//! DB-backed: a run queued with an A2A task and context reads back with
//! both. Fails when `DATABASE_URL` is unset.

use systemprompt_agent_runs::repository::NewAgentRun;
use systemprompt_identifiers::{AgentRunId, ContextId, TaskId};

use crate::repository::{fixture, request};

#[tokio::test]
async fn run_keeps_its_task_and_context() {
    let f = fixture().await;
    let (id, task_id, context_id) = (
        AgentRunId::generate(),
        TaskId::generate(),
        ContextId::generate(),
    );
    f.repo
        .insert_run(NewAgentRun {
            id: &id,
            origin_kind: &f.origin_kind,
            origin_id: "origin-a2a",
            user_id: &f.user_id,
            request: &request(),
            not_before: None,
            task_id: Some(&task_id),
            context_id: Some(&context_id),
        })
        .await
        .expect("insert run");
    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.task_id, Some(task_id));
    assert_eq!(row.context_id, Some(context_id));

    let plain = crate::repository::queue(&f).await;
    let row = f.repo.find_run(&plain).await.expect("find").expect("row");
    assert_eq!((row.task_id, row.context_id), (None, None));
}
