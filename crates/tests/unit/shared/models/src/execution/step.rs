use systemprompt_identifiers::{SkillId, TaskId};
use systemprompt_models::execution::{
    ExecutionStep, PlannedTool, StepContent, StepId, StepStatus, StepType,
};

// ============================================================================
// StepId Tests
// ============================================================================

#[test]
fn step_id_new_generates_unique_ids() {
    let id1 = StepId::new();
    let id2 = StepId::new();
    assert_ne!(id1.as_str(), id2.as_str());
}

#[test]
fn step_id_as_str_returns_inner_value() {
    let id = StepId::from("test-step-id".to_string());
    assert_eq!(id.as_str(), "test-step-id");
}

#[test]
fn step_id_display_matches_inner_value() {
    let id = StepId::from("display-id".to_string());
    assert_eq!(format!("{}", id), "display-id");
}

#[test]
fn step_id_default_generates_uuid() {
    let id = StepId::default();
    assert!(!id.as_str().is_empty());
}

#[test]
fn step_id_from_string() {
    let id = StepId::from("custom-id".to_string());
    assert_eq!(id.as_str(), "custom-id");
}

#[test]
fn step_id_clone_preserves_value() {
    let id = StepId::from("clone-me".to_string());
    let cloned = id.clone();
    assert_eq!(id.as_str(), cloned.as_str());
}

#[test]
fn step_id_equality() {
    let id1 = StepId::from("same".to_string());
    let id2 = StepId::from("same".to_string());
    assert_eq!(id1, id2);
}

// ============================================================================
// StepStatus Tests
// ============================================================================

#[test]
fn step_status_default_is_pending() {
    let status = StepStatus::default();
    assert_eq!(status, StepStatus::Pending);
}

#[test]
fn step_status_display_pending() {
    assert_eq!(format!("{}", StepStatus::Pending), "pending");
}

#[test]
fn step_status_display_in_progress() {
    assert_eq!(format!("{}", StepStatus::InProgress), "in_progress");
}

#[test]
fn step_status_display_completed() {
    assert_eq!(format!("{}", StepStatus::Completed), "completed");
}

#[test]
fn step_status_display_failed() {
    assert_eq!(format!("{}", StepStatus::Failed), "failed");
}

#[test]
fn step_status_from_str_pending() {
    let status: StepStatus = "pending".parse().unwrap();
    assert_eq!(status, StepStatus::Pending);
}

#[test]
fn step_status_from_str_in_progress() {
    let status: StepStatus = "in_progress".parse().unwrap();
    assert_eq!(status, StepStatus::InProgress);
}

#[test]
fn step_status_from_str_running_alias() {
    let status: StepStatus = "running".parse().unwrap();
    assert_eq!(status, StepStatus::InProgress);
}

#[test]
fn step_status_from_str_active_alias() {
    let status: StepStatus = "active".parse().unwrap();
    assert_eq!(status, StepStatus::InProgress);
}

#[test]
fn step_status_from_str_completed() {
    let status: StepStatus = "completed".parse().unwrap();
    assert_eq!(status, StepStatus::Completed);
}

#[test]
fn step_status_from_str_done_alias() {
    let status: StepStatus = "done".parse().unwrap();
    assert_eq!(status, StepStatus::Completed);
}

#[test]
fn step_status_from_str_success_alias() {
    let status: StepStatus = "success".parse().unwrap();
    assert_eq!(status, StepStatus::Completed);
}

#[test]
fn step_status_from_str_failed() {
    let status: StepStatus = "failed".parse().unwrap();
    assert_eq!(status, StepStatus::Failed);
}

#[test]
fn step_status_from_str_error_alias() {
    let status: StepStatus = "error".parse().unwrap();
    assert_eq!(status, StepStatus::Failed);
}

#[test]
fn step_status_from_str_invalid_returns_error() {
    let result: Result<StepStatus, _> = "unknown_status".parse();
    assert!(result.is_err());
}

#[test]
fn step_status_from_str_case_insensitive() {
    let status: StepStatus = "PENDING".parse().unwrap();
    assert_eq!(status, StepStatus::Pending);
}

// ============================================================================
// StepType Tests
// ============================================================================

#[test]
fn step_type_default_is_understanding() {
    let step_type = StepType::default();
    assert_eq!(step_type, StepType::Understanding);
}

#[test]
fn step_type_display_understanding() {
    assert_eq!(format!("{}", StepType::Understanding), "understanding");
}

#[test]
fn step_type_display_planning() {
    assert_eq!(format!("{}", StepType::Planning), "planning");
}

#[test]
fn step_type_display_skill_usage() {
    assert_eq!(format!("{}", StepType::SkillUsage), "skill_usage");
}

#[test]
fn step_type_display_tool_execution() {
    assert_eq!(format!("{}", StepType::ToolExecution), "tool_execution");
}

#[test]
fn step_type_display_completion() {
    assert_eq!(format!("{}", StepType::Completion), "completion");
}

#[test]
fn step_type_from_str_understanding() {
    let st: StepType = "understanding".parse().unwrap();
    assert_eq!(st, StepType::Understanding);
}

#[test]
fn step_type_from_str_tool_execution_alt() {
    let st: StepType = "toolexecution".parse().unwrap();
    assert_eq!(st, StepType::ToolExecution);
}

#[test]
fn step_type_from_str_invalid_returns_error() {
    let result: Result<StepType, _> = "invalid_type".parse();
    assert!(result.is_err());
}

// ============================================================================
// StepContent Tests
// ============================================================================

#[test]
fn step_content_understanding_step_type() {
    let content = StepContent::understanding();
    assert_eq!(content.step_type(), StepType::Understanding);
}

#[test]
fn step_content_understanding_title() {
    let content = StepContent::understanding();
    assert_eq!(content.title(), "Analyzing request...");
}

#[test]
fn step_content_understanding_is_instant() {
    let content = StepContent::understanding();
    assert!(content.is_instant());
}

#[test]
fn step_content_understanding_has_no_tool_name() {
    let content = StepContent::understanding();
    assert!(content.tool_name().is_none());
}

#[test]
fn step_content_planning_with_reasoning() {
    let content = StepContent::planning(Some("think about it".to_string()), None);
    assert_eq!(content.reasoning(), Some("think about it"));
}

#[test]
fn step_content_planning_with_planned_tools() {
    let tools = vec![PlannedTool {
        tool_name: "search".to_string(),
        arguments: serde_json::json!({"q": "test"}),
    }];
    let content = StepContent::planning(None, Some(tools));
    let planned = content.planned_tools().unwrap();
    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].tool_name, "search");
}

#[test]
fn step_content_planning_step_type() {
    let content = StepContent::planning(None, None);
    assert_eq!(content.step_type(), StepType::Planning);
}

#[test]
fn step_content_planning_is_instant() {
    let content = StepContent::planning(None, None);
    assert!(content.is_instant());
}

#[test]
fn step_content_skill_usage_step_type() {
    let content = StepContent::skill_usage(SkillId::new("s1".to_string()), "MySkill");
    assert_eq!(content.step_type(), StepType::SkillUsage);
}

#[test]
fn step_content_skill_usage_tool_name_returns_skill_name() {
    let content = StepContent::skill_usage(SkillId::new("s1".to_string()), "MySkill");
    assert_eq!(content.tool_name(), Some("MySkill"));
}

#[test]
fn step_content_skill_usage_title_contains_skill_name() {
    let content = StepContent::skill_usage(SkillId::new("s1".to_string()), "Research");
    assert!(content.title().contains("Research"));
}

#[test]
fn step_content_tool_execution_step_type() {
    let content = StepContent::tool_execution("my_tool", serde_json::json!({}));
    assert_eq!(content.step_type(), StepType::ToolExecution);
}

#[test]
fn step_content_tool_execution_is_not_instant() {
    let content = StepContent::tool_execution("my_tool", serde_json::json!({}));
    assert!(!content.is_instant());
}

#[test]
fn step_content_tool_execution_tool_name() {
    let content = StepContent::tool_execution("search", serde_json::json!({}));
    assert_eq!(content.tool_name(), Some("search"));
}

#[test]
fn step_content_tool_execution_arguments() {
    let args = serde_json::json!({"query": "test"});
    let content = StepContent::tool_execution("search", args.clone());
    assert_eq!(content.tool_arguments(), Some(&args));
}

#[test]
fn step_content_tool_execution_result_initially_none() {
    let content = StepContent::tool_execution("tool", serde_json::json!({}));
    assert!(content.tool_result().is_none());
}

#[test]
fn step_content_tool_execution_with_tool_result() {
    let content = StepContent::tool_execution("tool", serde_json::json!({}));
    let result = serde_json::json!({"output": "done"});
    let content = content.with_tool_result(result.clone());
    assert_eq!(content.tool_result(), Some(&result));
}

#[test]
fn step_content_with_tool_result_on_non_tool_is_noop() {
    let content = StepContent::understanding();
    let content = content.with_tool_result(serde_json::json!("ignored"));
    assert!(content.tool_result().is_none());
}

#[test]
fn step_content_tool_execution_title_contains_tool_name() {
    let content = StepContent::tool_execution("web_search", serde_json::json!({}));
    assert!(content.title().contains("web_search"));
}

#[test]
fn step_content_completion_step_type() {
    let content = StepContent::completion();
    assert_eq!(content.step_type(), StepType::Completion);
}

#[test]
fn step_content_completion_is_instant() {
    let content = StepContent::completion();
    assert!(content.is_instant());
}

#[test]
fn step_content_completion_title() {
    let content = StepContent::completion();
    assert_eq!(content.title(), "Complete");
}

#[test]
fn step_content_understanding_no_reasoning() {
    let content = StepContent::understanding();
    assert!(content.reasoning().is_none());
}

#[test]
fn step_content_understanding_no_planned_tools() {
    let content = StepContent::understanding();
    assert!(content.planned_tools().is_none());
}

#[test]
fn step_content_tool_execution_no_reasoning() {
    let content = StepContent::tool_execution("t", serde_json::json!({}));
    assert!(content.reasoning().is_none());
}

#[test]
fn step_content_completion_no_tool_arguments() {
    let content = StepContent::completion();
    assert!(content.tool_arguments().is_none());
}

// ============================================================================
// ExecutionStep Tests
// ============================================================================

fn test_task_id() -> TaskId {
    TaskId::new("test-task-123".to_string())
}

#[test]
fn execution_step_understanding_is_completed_immediately() {
    let step = ExecutionStep::understanding(test_task_id());
    assert_eq!(step.status, StepStatus::Completed);
    assert!(step.completed_at.is_some());
    assert_eq!(step.duration_ms, Some(0));
}

#[test]
fn execution_step_understanding_step_type() {
    let step = ExecutionStep::understanding(test_task_id());
    assert_eq!(step.step_type(), StepType::Understanding);
}

#[test]
fn execution_step_planning_is_completed_immediately() {
    let step = ExecutionStep::planning(test_task_id(), None, None);
    assert_eq!(step.status, StepStatus::Completed);
}

#[test]
fn execution_step_planning_with_reasoning() {
    let step = ExecutionStep::planning(test_task_id(), Some("reasoning text".to_string()), None);
    assert_eq!(step.reasoning(), Some("reasoning text"));
}

#[test]
fn execution_step_tool_execution_starts_in_progress() {
    let step = ExecutionStep::tool_execution(test_task_id(), "my_tool", serde_json::json!({}));
    assert_eq!(step.status, StepStatus::InProgress);
    assert!(step.completed_at.is_none());
    assert!(step.duration_ms.is_none());
}

#[test]
fn execution_step_tool_execution_tool_name() {
    let step = ExecutionStep::tool_execution(test_task_id(), "search", serde_json::json!({}));
    assert_eq!(step.tool_name(), Some("search"));
}

#[test]
fn execution_step_tool_execution_arguments() {
    let args = serde_json::json!({"key": "value"});
    let step = ExecutionStep::tool_execution(test_task_id(), "tool", args.clone());
    assert_eq!(step.tool_arguments(), Some(&args));
}

#[test]
fn execution_step_completion_is_completed_immediately() {
    let step = ExecutionStep::completion(test_task_id());
    assert_eq!(step.status, StepStatus::Completed);
    assert!(step.completed_at.is_some());
}

#[test]
fn execution_step_skill_usage() {
    let step = ExecutionStep::skill_usage(
        test_task_id(),
        SkillId::new("skill-1".to_string()),
        "MySkill",
    );
    assert_eq!(step.step_type(), StepType::SkillUsage);
    assert_eq!(step.tool_name(), Some("MySkill"));
}

#[test]
fn execution_step_new_assigns_unique_step_id() {
    let step1 = ExecutionStep::understanding(test_task_id());
    let step2 = ExecutionStep::understanding(test_task_id());
    assert_ne!(step1.step_id, step2.step_id);
}

#[test]
fn execution_step_new_preserves_task_id() {
    let task_id = test_task_id();
    let step = ExecutionStep::understanding(task_id.clone());
    assert_eq!(step.task_id, task_id);
}

#[test]
fn execution_step_new_has_no_error_message() {
    let step = ExecutionStep::understanding(test_task_id());
    assert!(step.error_message.is_none());
}

#[test]
fn execution_step_complete_sets_completed_status() {
    let mut step = ExecutionStep::tool_execution(test_task_id(), "tool", serde_json::json!({}));
    step.complete(None);
    assert_eq!(step.status, StepStatus::Completed);
    assert!(step.completed_at.is_some());
    assert!(step.duration_ms.is_some());
}

#[test]
fn execution_step_complete_with_result() {
    let mut step = ExecutionStep::tool_execution(test_task_id(), "tool", serde_json::json!({}));
    let result = serde_json::json!({"output": "success"});
    step.complete(Some(result.clone()));
    assert_eq!(step.tool_result(), Some(&result));
}

#[test]
fn execution_step_complete_without_result() {
    let mut step = ExecutionStep::tool_execution(test_task_id(), "tool", serde_json::json!({}));
    step.complete(None);
    assert!(step.tool_result().is_none());
}

#[test]
fn execution_step_fail_sets_failed_status() {
    let mut step = ExecutionStep::tool_execution(test_task_id(), "tool", serde_json::json!({}));
    step.fail("something went wrong".to_string());
    assert_eq!(step.status, StepStatus::Failed);
}

#[test]
fn execution_step_fail_sets_completed_at() {
    let mut step = ExecutionStep::tool_execution(test_task_id(), "tool", serde_json::json!({}));
    step.fail("error".to_string());
    assert!(step.completed_at.is_some());
}

#[test]
fn execution_step_fail_sets_duration() {
    let mut step = ExecutionStep::tool_execution(test_task_id(), "tool", serde_json::json!({}));
    step.fail("error".to_string());
    assert!(step.duration_ms.is_some());
}

#[test]
fn execution_step_fail_sets_error_message() {
    let mut step = ExecutionStep::tool_execution(test_task_id(), "tool", serde_json::json!({}));
    step.fail("something went wrong".to_string());
    assert_eq!(step.error_message, Some("something went wrong".to_string()));
}

#[test]
fn execution_step_title_delegates_to_content() {
    let step = ExecutionStep::completion(test_task_id());
    assert_eq!(step.title(), "Complete");
}

#[test]
fn execution_step_started_at_is_set() {
    let step = ExecutionStep::understanding(test_task_id());
    let now = chrono::Utc::now();
    let diff = (now - step.started_at).num_seconds().abs();
    assert!(diff < 2);
}
