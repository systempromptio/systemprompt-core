//! DB-backed: a run queued through `RepositoryRunLauncher::enqueue` is
//! claimed by a `ClaimLoop` driving `A2aRunHandler` (host pinned to the
//! `/bin/sh` fake), executed, finished, and its task completed. The loop
//! claims the fixture's own origin kind so it never touches other rows.
//! Fails when `DATABASE_URL` is unset.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_agent_runs::RepositoryRunLauncher;
use systemprompt_agent_runs::credentials::RunCredentialService;
use systemprompt_agent_runs::repository::RunCredentialRepositories;
use systemprompt_agent_runs::runner::{A2aRunHandler, A2aRunnerConfig};
use systemprompt_agent_runs::service::{ClaimLoop, claimer_id};
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{AgentRunLaunch, AgentRunLauncher};
use tokio_util::sync::CancellationToken;

use crate::execute_support::ShHost;
use crate::repository::{fixture, request};
use systemprompt_identifiers::AgentRunOriginId;

#[tokio::test]
async fn claim_loop_runs_an_enqueued_a2a_run_to_completion() {
    let f = fixture().await;
    let pool = test_db_pool().await;
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
    let run_id = RepositoryRunLauncher::new(f.repo.clone())
        .enqueue(AgentRunLaunch {
            user_id: f.user_id.clone(),
            task_id: task_id.clone(),
            context_id,
            request: serde_json::to_value(request()).expect("request json"),
            origin_kind: f.origin_kind.clone(),
            origin_id: AgentRunOriginId::new(task_id.to_string()),
        })
        .await
        .expect("enqueue");

    let data = std::env::temp_dir().join(format!("a2a-runner-{run_id}"));
    let config = A2aRunnerConfig::new("http://127.0.0.1:9", "https://issuer.test", &data);
    let host = ShHost {
        script: "echo done".to_owned(),
        keep: &[],
    };
    let handler = A2aRunHandler::new(
        RunCredentialService::new(
            RunCredentialRepositories::new(&pool),
            "https://issuer.test",
            systemprompt_users::ApiKeyLifetime::unbounded(),
            systemprompt_test_fixtures::test_signing_keys(),
        ),
        config,
    )
    .with_host(Arc::new(host));
    let mut claim = ClaimLoop::new(f.repo.clone(), &f.origin_kind, claimer_id("test"));
    claim.poll_every = Duration::from_millis(100);
    let shutdown = CancellationToken::new();
    let looping = tokio::spawn(claim.run(shutdown.clone(), Arc::new(handler)));

    let row = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let row = f.repo.find_run(&run_id).await.expect("find").expect("row");
            if row.finished_at.is_some() {
                return row;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("run finished");
    shutdown.cancel();
    looping.await.expect("loop joined");

    assert_eq!(row.status, "succeeded");
    assert!(row.api_key_id.is_some());
    let state: String = sqlx::query_scalar("SELECT status FROM agent_tasks WHERE task_id = $1")
        .bind(task_id.as_str())
        .fetch_one(f.repo.write_pool())
        .await
        .expect("task status");
    assert_eq!(state, "TASK_STATE_COMPLETED");
    assert!(f.repo.count_events(&run_id).await.expect("events") > 0);
    let _ = std::fs::remove_dir_all(data);
}
