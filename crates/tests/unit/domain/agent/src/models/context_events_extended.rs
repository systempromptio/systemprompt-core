use chrono::Utc;
use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part, TextPart};
use systemprompt_agent::models::context::{ContextKind, ContextStateEvent};
use systemprompt_identifiers::{
    AgentName, ArtifactId, ContextId, McpExecutionId, SessionId, SkillId, TaskId, TraceId,
};
use systemprompt_models::UserContext;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::fixture_user_id;

const CONTEXT_ID_1: &str = "10000000-0000-4000-8000-000000000001";
const CONTEXT_ID_2: &str = "20000000-0000-4000-8000-000000000002";

fn minimal_artifact() -> Artifact {
    Artifact {
        id: ArtifactId::new("art-1"),
        title: Some("result".to_string()),
        description: None,
        parts: vec![Part::Text(TextPart {
            text: "content".to_string(),
        })],
        extensions: vec![],
        metadata: ArtifactMetadata::new(
            "text".to_string(),
            ContextId::new(CONTEXT_ID_1),
            TaskId::new("task-art"),
        ),
    }
}

#[test]
fn context_state_event_artifact_created_context_id() {
    let event = ContextStateEvent::ArtifactCreated {
        artifact: minimal_artifact(),
        task_id: TaskId::new("task-art"),
        context_id: ContextId::new(CONTEXT_ID_1),
        timestamp: Utc::now(),
    };
    assert_eq!(event.context_id(), Some(CONTEXT_ID_1));
    assert!(event.timestamp() <= Utc::now());
}

#[test]
fn context_state_event_artifact_created_serialize() {
    let event = ContextStateEvent::ArtifactCreated {
        artifact: minimal_artifact(),
        task_id: TaskId::new("task-serial"),
        context_id: ContextId::new(CONTEXT_ID_2),
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("artifact_created"));
    assert!(json.contains(CONTEXT_ID_2));
}

#[test]
fn context_state_event_skill_loaded_has_context_id() {
    let ctx = RequestContext::new(
        SessionId::new("sess-1"),
        TraceId::new("trace-1"),
        ContextId::new(CONTEXT_ID_1),
        AgentName::new("test-agent"),
    );
    let event = ContextStateEvent::SkillLoaded {
        skill_id: SkillId::new("skill-1"),
        skill_name: "MySkill".to_string(),
        description: "Does things".to_string(),
        request_context: ctx,
        tool_name: Some("tool_fn".to_string()),
        timestamp: Utc::now(),
    };
    assert_eq!(event.context_id(), Some(CONTEXT_ID_1));
}

#[test]
fn context_state_event_skill_loaded_no_tool_name() {
    let ctx = RequestContext::new(
        SessionId::new("sess-2"),
        TraceId::new("trace-2"),
        ContextId::new(CONTEXT_ID_2),
        AgentName::new("test-agent"),
    );
    let event = ContextStateEvent::SkillLoaded {
        skill_id: SkillId::new("skill-2"),
        skill_name: "AnotherSkill".to_string(),
        description: "Another one".to_string(),
        request_context: ctx,
        tool_name: None,
        timestamp: Utc::now(),
    };
    assert_eq!(event.context_id(), Some(CONTEXT_ID_2));
}

#[test]
fn context_state_event_tool_execution_with_artifact() {
    let event = ContextStateEvent::ToolExecutionCompleted {
        context_id: ContextId::new(CONTEXT_ID_1),
        execution_id: McpExecutionId::new("exec-with-art"),
        tool_name: "fetch_data".to_string(),
        server_name: "data-server".to_string(),
        output: Some("fetched data".to_string()),
        artifact: Some(minimal_artifact()),
        status: "success".to_string(),
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("tool_execution_completed"));
    assert!(json.contains("fetch_data"));
}

#[test]
fn context_state_event_tool_execution_without_output() {
    let event = ContextStateEvent::ToolExecutionCompleted {
        context_id: ContextId::new(CONTEXT_ID_1),
        execution_id: McpExecutionId::new("exec-no-out"),
        tool_name: "void_tool".to_string(),
        server_name: "srv".to_string(),
        output: None,
        artifact: None,
        status: "success".to_string(),
        timestamp: Utc::now(),
    };
    assert_eq!(event.context_id(), Some(CONTEXT_ID_1));
}

#[test]
fn context_state_event_context_created_serialize() {
    let event = ContextStateEvent::ContextCreated {
        context_id: ContextId::new(CONTEXT_ID_1),
        context: UserContext {
            context_id: ContextId::new(CONTEXT_ID_1),
            name: "My Context".to_string(),
            kind: ContextKind::User,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: fixture_user_id(),
        },
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("context_created"));
    assert!(json.contains("My Context"));
}

#[test]
fn context_state_event_current_agent_none_name() {
    let event = ContextStateEvent::CurrentAgent {
        context_id: ContextId::new(CONTEXT_ID_2),
        agent_name: None,
        timestamp: Utc::now(),
    };
    assert_eq!(event.context_id(), Some(CONTEXT_ID_2));
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("current_agent"));
}

#[test]
fn context_state_event_task_status_changed_timestamp() {
    let now = Utc::now();
    let event = ContextStateEvent::TaskStatusChanged {
        task: systemprompt_agent::Task::default(),
        context_id: ContextId::new(CONTEXT_ID_1),
        timestamp: now,
    };
    assert_eq!(event.timestamp(), now);
}

#[test]
fn context_state_event_heartbeat_timestamp() {
    let now = Utc::now();
    let event = ContextStateEvent::Heartbeat { timestamp: now };
    assert_eq!(event.timestamp(), now);
    assert!(event.context_id().is_none());
}
