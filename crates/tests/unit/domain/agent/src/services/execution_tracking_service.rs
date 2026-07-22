// DB-backed tests for ExecutionTrackingService: the typed track_* entry
// points, async tracking handles, completion / planning-completion / failure
// transitions, and lookups by task and step.

use std::sync::Arc;

use serde_json::json;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::execution_tracking::ExecutionTrackingService;
use systemprompt_identifiers::{SkillId, TaskId};
use systemprompt_models::{PlannedTool, StepStatus};

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

async fn setup() -> Option<(
    ExecutionTrackingService,
    TaskId,
    systemprompt_agent::repository::A2ARepositories,
)> {
    let pool = try_pool().await?;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let repo = Arc::new(ExecutionStepRepository::new(&pool).expect("exec repo"));
    Some((ExecutionTrackingService::new(repo), task_id, r))
}

#[tokio::test]
async fn track_understanding_and_completion_persist_steps() {
    let Some((svc, task_id, r)) = setup().await else {
        return;
    };

    let understanding = svc
        .track_understanding(task_id.clone())
        .await
        .expect("understanding");
    let completion = svc
        .track_completion(task_id.clone())
        .await
        .expect("completion");

    let steps = svc.list_steps_by_task(&task_id).await.expect("list");
    assert_eq!(steps.len(), 2);
    assert!(steps.iter().any(|s| s.step_id == understanding.step_id));
    assert!(steps.iter().any(|s| s.step_id == completion.step_id));

    let found = svc
        .find_step(&understanding.step_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.task_id, task_id);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn track_skill_usage_records_skill_step() {
    let Some((svc, task_id, r)) = setup().await else {
        return;
    };

    let step = svc
        .track_skill_usage(task_id.clone(), SkillId::new("skill-x"), "Skill X")
        .await
        .expect("skill usage");

    let found = svc
        .find_step(&step.step_id)
        .await
        .expect("find")
        .expect("present");
    let rendered = serde_json::to_string(&found.content).expect("content json");
    assert!(rendered.contains("skill-x"));
    assert!(rendered.contains("Skill X"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn tool_execution_lifecycle_complete_and_fail() {
    let Some((svc, task_id, r)) = setup().await else {
        return;
    };

    let (tracked_ok, _) = svc
        .track_tool_execution(task_id.clone(), "tool-a", json!({"arg": 1}))
        .await
        .expect("track ok");
    let ok_step_id = tracked_ok.step_id.clone();
    svc.complete(tracked_ok, Some(json!({"out": true})))
        .await
        .expect("complete");

    let (tracked_fail, _) = svc
        .track_tool_execution(task_id.clone(), "tool-b", json!({"arg": 2}))
        .await
        .expect("track fail");
    let fail_step_id = tracked_fail.step_id.clone();
    svc.fail(&tracked_fail, "tool exploded".to_owned())
        .await
        .expect("fail");

    let completed = svc
        .find_step(&ok_step_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(completed.status, StepStatus::Completed);

    let failed = svc
        .find_step(&fail_step_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(failed.status, StepStatus::Failed);
    assert_eq!(failed.error_message.as_deref(), Some("tool exploded"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn planning_lifecycle_completes_with_reasoning_and_tools() {
    let Some((svc, task_id, r)) = setup().await else {
        return;
    };

    let (tracked, _) = svc
        .track_planning_async(task_id.clone(), Some("initial".to_owned()), None)
        .await
        .expect("track planning");
    let step_id = tracked.step_id.clone();

    let planned = vec![PlannedTool {
        tool_name: "search".to_owned(),
        arguments: json!({"q": "look things up"}),
    }];
    let step = svc
        .complete_planning(tracked, Some("final reasoning".to_owned()), Some(planned))
        .await
        .expect("complete planning");
    assert_eq!(step.step_id, step_id);
    assert_eq!(step.status, StepStatus::Completed);
    let rendered = serde_json::to_string(&step.content).expect("content json");
    assert!(rendered.contains("final reasoning"));
    assert!(rendered.contains("search"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn fail_step_and_fail_in_progress_mark_open_steps() {
    let Some((svc, task_id, r)) = setup().await else {
        return;
    };

    let (tracked, step) = svc
        .track_tool_execution(task_id.clone(), "tool-c", json!({}))
        .await
        .expect("track");
    svc.fail_step(
        &tracked.step_id,
        step.started_at,
        "direct failure".to_owned(),
    )
    .await
    .expect("fail step");

    let failed = svc
        .find_step(&tracked.step_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(failed.status, StepStatus::Failed);

    svc.track_tool_execution(task_id.clone(), "tool-d", json!({}))
        .await
        .expect("track open");
    let swept = svc
        .fail_in_progress_steps(&task_id, "shutdown")
        .await
        .expect("sweep");
    assert_eq!(swept, 1);

    r.tasks.delete_task(&task_id).await.ok();
}
