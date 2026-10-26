//! DB-backed: a run linked to an A2A task moves the task in the same
//! transaction as its own row, and the launcher queues, finds and cancels
//! task-backed runs, and a final-attempt reclaim fails the
//! task. Fails when `DATABASE_URL` is unset.

use std::path::Path;
use std::time::Duration;

use systemprompt_agent_runs::repository::{NewAgentRun, RunStarted};
use systemprompt_agent_runs::{ErrorClass, RepositoryRunLauncher, RunOutcome, RunStatus};
use systemprompt_identifiers::{AgentRunId, ContextId, TaskId};
use systemprompt_traits::{AgentRunLaunch, AgentRunLaunchError, AgentRunLauncher};

use crate::repository::{Fixture, expire_lease, fixture, request};
use systemprompt_identifiers::AgentRunOriginId;

async fn insert_task(f: &Fixture) -> (TaskId, ContextId) {
    let (task_id, context_id) = (TaskId::generate(), ContextId::generate());
    sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, 'runs')")
        .bind(context_id.as_str())
        .bind(f.user_id.as_str())
        .execute(f.repo.write_pool())
        .await
        .expect("insert context");
    sqlx::query("INSERT INTO agent_tasks (task_id, context_id, user_id) VALUES ($1, $2, $3)")
        .bind(task_id.as_str())
        .bind(context_id.as_str())
        .bind(f.user_id.as_str())
        .execute(f.repo.write_pool())
        .await
        .expect("insert task");
    (task_id, context_id)
}

async fn task_state(f: &Fixture, task_id: &TaskId) -> String {
    sqlx::query_scalar("SELECT status FROM agent_tasks WHERE task_id = $1")
        .bind(task_id.as_str())
        .fetch_one(f.repo.write_pool())
        .await
        .expect("task status")
}

async fn queue_for(f: &Fixture, task_id: &TaskId, context_id: &ContextId) -> AgentRunId {
    let id = AgentRunId::generate();
    f.repo
        .insert_run(NewAgentRun {
            id: &id,
            origin_kind: &f.origin_kind,
            origin_id: task_id.as_str(),
            user_id: &f.user_id,
            request: &request(),
            not_before: None,
            task_id: Some(task_id),
            context_id: Some(context_id),
        })
        .await
        .expect("insert run");
    id
}

fn succeeded() -> RunOutcome {
    RunOutcome {
        status: RunStatus::Succeeded,
        error_class: ErrorClass::Ok,
        session_id: None,
        cost_microdollars: 0,
        input_tokens: 0,
        output_tokens: 0,
        turns: 1,
        duration_ms: 5,
        structured_output: None,
        exit_code: Some(0),
        error: None,
        final_text: Some("done".into()),
    }
}

#[tokio::test]
async fn run_lifecycle_moves_its_task() {
    let f = fixture().await;
    let (task_id, context_id) = insert_task(&f).await;
    let id = queue_for(&f, &task_id, &context_id).await;
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_SUBMITTED");

    let started = RunStarted {
        run_dir: Path::new("/tmp/run"),
        pid: Some(1),
        gateway_session_id: None,
        api_key_id: None,
    };
    f.repo.update_started(&id, started).await.expect("started");
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_WORKING");

    assert!(
        f.repo
            .finish(&id, &succeeded(), None)
            .await
            .expect("finish")
    );
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_COMPLETED");

    let mut again = succeeded();
    again.status = RunStatus::Failed;
    assert!(
        !f.repo
            .finish(&id, &again, None)
            .await
            .expect("second finish")
    );
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_COMPLETED");
}

#[tokio::test]
async fn finish_on_a_terminal_task_still_records_the_run() {
    let f = fixture().await;
    let (task_id, context_id) = insert_task(&f).await;
    let id = queue_for(&f, &task_id, &context_id).await;
    sqlx::query("UPDATE agent_tasks SET status = 'TASK_STATE_CANCELED' WHERE task_id = $1")
        .bind(task_id.as_str())
        .execute(f.repo.write_pool())
        .await
        .expect("cancel task");
    assert!(
        f.repo
            .finish(&id, &succeeded(), None)
            .await
            .expect("finish")
    );
    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "succeeded");
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_CANCELED");
}

#[tokio::test]
async fn launcher_queues_finds_and_cancels_a_task_run() {
    let f = fixture().await;
    let (task_id, context_id) = insert_task(&f).await;
    let launcher = RepositoryRunLauncher::new(f.repo.clone());
    let launch = AgentRunLaunch {
        user_id: f.user_id.clone(),
        task_id: task_id.clone(),
        context_id: context_id.clone(),
        request: serde_json::to_value(request()).expect("request json"),
        origin_kind: f.origin_kind.clone(),
        origin_id: AgentRunOriginId::new(task_id.to_string()),
    };
    let run_id = launcher.enqueue(launch.clone()).await.expect("enqueue");
    assert_eq!(
        launcher.run_for_task(&task_id).await.expect("lookup"),
        Some(run_id.clone())
    );

    launcher.cancel(&run_id).await.expect("cancel");
    let row = f.repo.find_run(&run_id).await.expect("find").expect("row");
    assert_eq!(row.status, "cancelled");
    assert!(row.cancel_requested_at.is_some());
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_CANCELED");

    let bad = AgentRunLaunch {
        request: serde_json::json!({"host": "nope"}),
        ..launch
    };
    assert!(matches!(
        launcher.enqueue(bad).await,
        Err(AgentRunLaunchError::InvalidRequest(_))
    ));
}

#[tokio::test]
async fn final_attempt_reclaim_fails_its_task() {
    let f = fixture().await;
    let (task_id, context_id) = insert_task(&f).await;
    let id = queue_for(&f, &task_id, &context_id).await;
    f.repo
        .claim_queued(&f.origin_kind, "a", Duration::from_secs(90))
        .await
        .expect("claim")
        .expect("row");
    expire_lease(&f, &id).await;
    assert_eq!(
        f.repo
            .reclaim_stale(&f.origin_kind, 1)
            .await
            .expect("reclaim"),
        vec![id.clone()]
    );
    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "failed");
    assert_eq!(row.error_class.as_deref(), Some("host_crashed"));
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_FAILED");
}
