//! A2A mappings: run status ⇄ task state, outcome artifacts, prompt
//! flattening and the streaming updates each run event produces.

use chrono::Utc;
use systemprompt_agent_runs::host::claude_code::ClaudeCode;
use systemprompt_agent_runs::{
    ErrorClass, ExitInfo, HeadlessHost, RunEvent, RunEventPayload, RunOutcome, RunRequest,
    RunStatus, RunnerLevel, TaskUpdate, task_events,
};
use systemprompt_identifiers::{AgentRunId, ContextId, HostSessionId, MessageId, TaskId};
use systemprompt_models::a2a::{Message, MessageRole, Part, TaskState};

const ALL: [RunStatus; 7] = [
    RunStatus::Queued,
    RunStatus::Preparing,
    RunStatus::Running,
    RunStatus::Succeeded,
    RunStatus::Failed,
    RunStatus::Cancelled,
    RunStatus::TimedOut,
];

#[test]
fn run_status_maps_to_task_state() {
    let mapped: Vec<TaskState> = ALL.into_iter().map(TaskState::from).collect();
    assert_eq!(
        mapped,
        vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::Working,
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Canceled,
            TaskState::Failed,
        ]
    );
}

#[test]
fn task_state_round_trips_through_run_status() {
    for state in [
        TaskState::Submitted,
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
    ] {
        let status = RunStatus::from_task_state(&state).expect("mapped");
        assert_eq!(TaskState::from(status), state);
    }
    for state in [
        TaskState::Rejected,
        TaskState::InputRequired,
        TaskState::AuthRequired,
    ] {
        assert_eq!(RunStatus::from_task_state(&state), None);
    }
}

#[test]
fn run_lifecycle_is_a_valid_task_lifecycle() {
    let submitted = TaskState::from(RunStatus::Queued);
    let working = TaskState::from(RunStatus::Running);
    assert!(submitted.can_transition_to(&working));
    for end in [
        RunStatus::Succeeded,
        RunStatus::Failed,
        RunStatus::Cancelled,
        RunStatus::TimedOut,
    ] {
        let terminal = TaskState::from(end);
        assert!(working.can_transition_to(&terminal), "{end:?}");
        assert!(submitted.can_transition_to(&terminal), "{end:?}");
        assert!(!terminal.can_transition_to(&working), "{end:?}");
    }
}

fn outcome() -> RunOutcome {
    RunOutcome {
        status: RunStatus::Succeeded,
        error_class: ErrorClass::Ok,
        session_id: None,
        cost_microdollars: 0,
        input_tokens: 0,
        output_tokens: 0,
        turns: 1,
        duration_ms: 10,
        structured_output: None,
        exit_code: Some(0),
        error: None,
        final_text: None,
    }
}

#[test]
fn outcome_artifacts_carry_data_then_text() {
    let (run, task, ctx) = (
        AgentRunId::generate(),
        TaskId::generate(),
        ContextId::generate(),
    );
    assert!(outcome().artifacts(&run, &task, &ctx).is_empty());

    let mut out = outcome();
    out.structured_output = Some(serde_json::json!({"count": 2}));
    out.final_text = Some("done".into());
    let artifacts = out.artifacts(&run, &task, &ctx);
    assert_eq!(artifacts.len(), 1);
    let artifact = &artifacts[0];
    assert_eq!(artifact.metadata.task_id, task);
    assert_eq!(artifact.metadata.context_id, ctx);
    assert_eq!(artifact.parts.len(), 2);
    assert_eq!(
        artifact.parts[0].as_data(),
        Some(&serde_json::json!({"count": 2}))
    );
    assert_eq!(artifact.parts[1].as_text(), Some("done"));

    let mut scalar = outcome();
    scalar.structured_output = Some(serde_json::json!(7));
    let parts = &scalar.artifacts(&run, &task, &ctx)[0].parts;
    assert_eq!(parts[0].as_data(), Some(serde_json::json!({"value": 7})));
}

#[test]
fn claude_outcome_final_text_is_last_assistant_text() {
    let events = vec![
        RunEventPayload::AssistantText {
            text: "first".into(),
        },
        RunEventPayload::AssistantText {
            text: "last".into(),
        },
    ];
    let exit = ExitInfo {
        code: Some(1),
        timed_out: false,
        cancelled: false,
        duration_ms: 1,
    };
    let out = ClaudeCode.outcome(exit, &events, &crate::repository::request());
    assert_eq!(out.final_text.as_deref(), Some("last"));
}

#[test]
fn prompt_flattens_parts_in_order() {
    let message = Message {
        role: MessageRole::User,
        parts: vec![
            Part::text("fix it"),
            Part::data(serde_json::json!({"k": 1})),
            Part::url("https://x/a.txt", "text/plain").with_filename("a.txt"),
        ],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };
    assert_eq!(
        RunRequest::prompt_from_message(&message),
        "fix it\n\n```json\n{\n  \"k\": 1\n}\n```\n\n[file: https://x/a.txt]"
    );
}

fn event(seq: i64, payload: RunEventPayload) -> RunEvent {
    RunEvent {
        seq,
        kind: payload.kind(),
        tool_name: None,
        tool_use_id: None,
        payload,
        created_at: Utc::now(),
    }
}

fn updates(payload: RunEventPayload) -> Vec<TaskUpdate> {
    let (run, task, ctx) = (
        AgentRunId::generate(),
        TaskId::generate(),
        ContextId::generate(),
    );
    task_events(&run, &task, &ctx, &event(3, payload))
}

fn result(is_error: bool, structured: Option<serde_json::Value>) -> RunEventPayload {
    RunEventPayload::Result {
        subtype: "success".into(),
        is_error,
        num_turns: 1,
        duration_ms: 1,
        input_tokens: 0,
        output_tokens: 0,
        cost_microdollars: 0,
        structured_output: structured,
        error: is_error.then(|| "boom".to_owned()),
    }
}

fn status_of(update: &TaskUpdate) -> TaskState {
    match update {
        TaskUpdate::Status(s) => s.status.state,
        TaskUpdate::Artifact(_) => panic!("expected a status update"),
    }
}

#[test]
fn task_events_per_kind() {
    let init = updates(RunEventPayload::Init {
        session_id: HostSessionId::try_new("s").expect("host session id"),
        model: "m".into(),
        tools: Vec::new(),
        cwd: None,
    });
    assert_eq!(
        init.iter().map(status_of).collect::<Vec<_>>(),
        vec![TaskState::Working]
    );

    let runner = updates(RunEventPayload::Runner {
        message: "preparing".into(),
        level: RunnerLevel::Info,
    });
    let TaskUpdate::Status(s) = &runner[0] else {
        panic!("status")
    };
    assert_eq!(
        s.status.message.as_ref().expect("message").parts[0].as_text(),
        Some("preparing")
    );

    let text = updates(RunEventPayload::AssistantText { text: "hi".into() });
    let TaskUpdate::Artifact(a) = &text[0] else {
        panic!("artifact")
    };
    assert_eq!(
        (a.artifact.parts[0].as_text(), a.append, a.last_chunk),
        (Some("hi"), false, true)
    );
    assert!(a.artifact.id.as_str().ends_with("-text-3"));

    let ok = updates(result(false, Some(serde_json::json!({"a": 1}))));
    assert_eq!(ok.len(), 2);
    assert!(matches!(&ok[0], TaskUpdate::Artifact(a) if a.last_chunk && !a.append));
    assert_eq!(status_of(&ok[1]), TaskState::Completed);

    let failed = updates(result(true, None));
    assert_eq!(
        failed.iter().map(status_of).collect::<Vec<_>>(),
        vec![TaskState::Failed]
    );

    assert!(updates(RunEventPayload::Stderr { line: "x".into() }).is_empty());
}
