//! Task and message construction on the non-streaming A2A path.
//!
//! These three helpers decide what a turn is attached to and what the client
//! gets back. Between them they settle whether a follow-up message continues
//! the conversation or starts a new one, and whether the client can match the
//! reply it receives to the request it sent.

use systemprompt_agent::models::a2a::{
    Message, MessageRole, Part, TaskState, TaskStatus, TextPart,
};
use systemprompt_agent::test_api::{new_submitted_task, resolve_agent_message, resolve_task_id};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

fn message_with(task_id: Option<TaskId>, metadata: Option<serde_json::Value>) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "ask".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id,
        context_id: ContextId::generate(),
        metadata,
        extensions: None,
        reference_task_ids: None,
    }
}

// Why: this is what makes a conversation a conversation. A follow-up carrying
// a task id must continue that task; generating a fresh one would split the
// exchange into unrelated tasks and lose the history the client is building on.
#[test]
fn a_message_carrying_a_task_id_continues_that_task() {
    let existing = TaskId::generate();

    let resolved = resolve_task_id(&message_with(Some(existing.clone()), None));

    assert_eq!(resolved, existing, "an existing task must be continued");
}

// Why: the converse. A first message has no task, so one is minted — and it
// must be unique per call, or two conversations would collide on one task row.
#[test]
fn a_message_with_no_task_id_starts_a_new_and_unique_task() {
    let first = resolve_task_id(&message_with(None, None));
    let second = resolve_task_id(&message_with(None, None));

    assert!(!first.as_str().is_empty());
    assert_ne!(
        first, second,
        "each new turn must get its own task id, not a shared one"
    );
}

// Why: a task starts life Submitted. Any other state would tell a polling
// client the work had already progressed, and `new_agent_message` metadata is
// what attributes the task to the agent that owns it.
#[test]
fn a_new_task_starts_submitted_and_is_attributed_to_its_agent() {
    let task_id = TaskId::generate();
    let context_id = ContextId::generate();

    let task = new_submitted_task(&task_id, &context_id, "writer");

    assert_eq!(task.id, task_id);
    assert_eq!(task.context_id, context_id);
    assert_eq!(
        task.status.state,
        TaskState::Submitted,
        "a task must not claim progress it has not made"
    );
    assert!(
        task.history.is_none() && task.artifacts.is_none(),
        "a submitted task has produced nothing yet"
    );
    assert!(
        task.metadata.is_some(),
        "the owning agent must be recorded on the task"
    );
    assert!(task.created_at.is_some() && task.last_modified.is_some());
}

// Why: when the pipeline already produced an agent message, that is the real
// reply. Rebuilding one from the raw response text would discard whatever the
// pipeline attached to it.
#[test]
fn an_agent_message_already_on_the_task_is_used_verbatim() {
    let task_id = TaskId::generate();
    let context_id = ContextId::generate();
    let mut task = new_submitted_task(&task_id, &context_id, "writer");
    let produced = Message {
        role: MessageRole::Agent,
        parts: vec![Part::Text(TextPart {
            text: "pipeline answer".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };
    task.status = TaskStatus {
        state: TaskState::Completed,
        message: Some(produced.clone()),
        timestamp: None,
    };

    let resolved = resolve_agent_message(&task, &message_with(None, None), "fallback text");

    assert_eq!(
        resolved.message_id, produced.message_id,
        "the pipeline's own message must be returned, not a rebuilt one"
    );
}

// Why: with no message on the task, one is synthesised from the response text
// and must be bound to the same task and context — an unbound reply cannot be
// filed against the conversation it answers.
#[test]
fn a_synthesised_reply_is_bound_to_its_task_and_context() {
    let task_id = TaskId::generate();
    let context_id = ContextId::generate();
    let task = new_submitted_task(&task_id, &context_id, "writer");

    let resolved = resolve_agent_message(&task, &message_with(None, None), "the answer");

    assert_eq!(resolved.role, MessageRole::Agent);
    assert_eq!(resolved.task_id, Some(task_id));
    assert_eq!(resolved.context_id, context_id);
    assert_eq!(
        resolved.parts,
        vec![Part::Text(TextPart {
            text: "the answer".to_owned()
        })],
        "the response text must be carried into the reply"
    );
}

// Why: `clientMessageId` is the client's own correlation handle. It sends one
// and matches the reply to the request by it. Dropping it on the synthesised
// path leaves an async client unable to tell which request a reply answers.
#[test]
fn the_clients_correlation_id_is_carried_into_the_synthesised_reply() {
    let task = new_submitted_task(&TaskId::generate(), &ContextId::generate(), "writer");
    let user = message_with(None, Some(serde_json::json!({"clientMessageId": "cm-42"})));

    let resolved = resolve_agent_message(&task, &user, "the answer");

    let metadata = resolved
        .metadata
        .expect("correlation metadata must survive");
    assert_eq!(
        metadata.get("clientMessageId"),
        Some(&serde_json::json!("cm-42")),
        "the client's correlation id must come back on the reply"
    );
}

// Why: the converse — a client that sent no correlation id must not be handed
// a fabricated one, and unrelated user metadata must not be copied wholesale
// onto an agent message.
#[test]
fn a_reply_carries_no_metadata_when_the_client_sent_no_correlation_id() {
    let task = new_submitted_task(&TaskId::generate(), &ContextId::generate(), "writer");
    let user = message_with(None, Some(serde_json::json!({"unrelated": "field"})));

    let resolved = resolve_agent_message(&task, &user, "the answer");

    assert!(
        resolved.metadata.is_none(),
        "only the correlation id crosses over, and there was none: {:?}",
        resolved.metadata
    );
}
