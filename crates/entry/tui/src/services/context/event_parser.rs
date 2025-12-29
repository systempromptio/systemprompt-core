use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info, warn};

use systemprompt_identifiers::ContextId;
use systemprompt_models::a2a::{Task, TaskState, TaskStatus};
use systemprompt_models::{A2AEvent, AgUiEvent, ContextEvent, SystemEvent};

use crate::messages::{ContextLifecycleEvent, ContextStreamTaskEvent, Message};

pub fn parse_and_dispatch(data: &str, message_tx: &UnboundedSender<Message>) {
    if data.is_empty() || is_heartbeat(data) {
        return;
    }

    match serde_json::from_str::<ContextEvent>(data) {
        Ok(context_event) => {
            info!(data = %data, "Parsed ContextEvent successfully");
            dispatch_context_event(context_event, message_tx);
        },
        Err(e) => try_parse_legacy_agui(data, &e, message_tx),
    }
}

fn is_heartbeat(data: &str) -> bool {
    data == "ping" || data == "heartbeat" || data.contains("\"type\":\"HEARTBEAT\"")
}

fn try_parse_legacy_agui(
    data: &str,
    original_error: &serde_json::Error,
    message_tx: &UnboundedSender<Message>,
) {
    match serde_json::from_str::<AgUiEvent>(data) {
        Ok(agui_event) => dispatch_legacy_agui_event(agui_event, message_tx),
        Err(_) => log_parse_failure(data, original_error),
    }
}

fn dispatch_legacy_agui_event(agui_event: AgUiEvent, message_tx: &UnboundedSender<Message>) {
    let event_type = agui_event.event_type();
    info!(event_type = ?event_type, "Received AG-UI event (legacy format)");
    let _ = message_tx.send(Message::AgUiEvent(agui_event));
}

fn log_parse_failure(data: &str, original_error: &serde_json::Error) {
    warn!(
        error = %original_error,
        data = %data,
        "Failed to parse as ContextEvent or AgUiEvent"
    );
}

fn dispatch_context_event(event: ContextEvent, message_tx: &UnboundedSender<Message>) {
    match event {
        ContextEvent::AgUi(agui_event) => {
            let event_type = agui_event.event_type();
            info!(event_type = ?event_type, "Dispatching AG-UI event to TUI");
            let _ = message_tx.send(Message::AgUiEvent(agui_event));
        },
        ContextEvent::A2A(a2a_event) => dispatch_a2a_event(*a2a_event, message_tx),
        ContextEvent::System(system_event) => dispatch_system_event(system_event, message_tx),
    }
}

fn dispatch_a2a_event(event: A2AEvent, message_tx: &UnboundedSender<Message>) {
    match event {
        A2AEvent::TaskSubmitted { payload, .. } => {
            handle_task_submitted(payload.task_id, payload.context_id, message_tx);
        },
        A2AEvent::TaskStatusUpdate { payload, .. } => {
            handle_task_status_update(
                payload.task_id,
                payload.context_id,
                payload.state,
                payload.message,
                message_tx,
            );
        },
        A2AEvent::ArtifactCreated { payload, .. } => {
            handle_artifact_created(*payload, message_tx);
        },
        _ => debug!("Unhandled A2A event type"),
    }
}

fn handle_artifact_created(
    payload: systemprompt_models::events::payloads::a2a::ArtifactCreatedPayload,
    message_tx: &UnboundedSender<Message>,
) {
    debug!(
        task_id = %payload.task_id,
        artifact_id = %payload.artifact.id,
        "A2A ArtifactCreated"
    );
    let _ = message_tx.send(Message::ArtifactsLoaded(vec![payload.artifact]));
}

fn handle_task_submitted(
    task_id: systemprompt_identifiers::TaskId,
    context_id: ContextId,
    message_tx: &UnboundedSender<Message>,
) {
    debug!(task_id = %task_id, "A2A TaskSubmitted");
    let task = create_task(task_id, context_id, TaskState::Submitted, None);
    let _ = message_tx.send(Message::ContextStreamTask(Box::new(
        ContextStreamTaskEvent::Created(task),
    )));
}

fn handle_task_status_update(
    task_id: systemprompt_identifiers::TaskId,
    context_id: ContextId,
    state: TaskState,
    message: Option<String>,
    message_tx: &UnboundedSender<Message>,
) {
    debug!(task_id = %task_id, state = ?state, "A2A TaskStatusUpdate");
    let status_message = message.map(|m| create_status_message(m, &context_id));
    let task = create_task(task_id, context_id, state, status_message);

    let event = if state == TaskState::Completed {
        ContextStreamTaskEvent::Completed(task)
    } else {
        ContextStreamTaskEvent::StatusChanged(task)
    };
    let _ = message_tx.send(Message::ContextStreamTask(Box::new(event)));
}

fn create_task(
    task_id: systemprompt_identifiers::TaskId,
    context_id: ContextId,
    state: TaskState,
    message: Option<systemprompt_models::a2a::Message>,
) -> Task {
    Task {
        id: task_id,
        context_id,
        status: TaskStatus {
            state,
            message,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: None,
        kind: "task".to_string(),
    }
}

fn create_status_message(
    text: String,
    context_id: &ContextId,
) -> systemprompt_models::a2a::Message {
    systemprompt_models::a2a::Message {
        role: "system".to_string(),
        parts: vec![systemprompt_models::a2a::Part::Text(
            systemprompt_models::a2a::TextPart { text },
        )],
        id: systemprompt_identifiers::MessageId::generate(),
        task_id: None,
        context_id: context_id.clone(),
        kind: "message".to_string(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn dispatch_system_event(event: SystemEvent, message_tx: &UnboundedSender<Message>) {
    match event {
        SystemEvent::ContextCreated { payload, .. } => handle_context_created(payload, message_tx),
        SystemEvent::ContextUpdated { payload, .. } => handle_context_updated(payload, message_tx),
        SystemEvent::ContextDeleted { payload, .. } => handle_context_deleted(payload, message_tx),
        SystemEvent::Connected { .. }
        | SystemEvent::Heartbeat { .. }
        | SystemEvent::ContextsSnapshot { .. } => debug!("System lifecycle/snapshot event"),
    }
}

fn handle_context_created(
    payload: systemprompt_models::events::payloads::system::ContextCreatedPayload,
    message_tx: &UnboundedSender<Message>,
) {
    debug!(context_id = %payload.context_id, "System ContextCreated");
    send_lifecycle(
        message_tx,
        ContextLifecycleEvent::Created {
            context_id: payload.context_id,
            name: Some(payload.name),
        },
    );
}

fn handle_context_updated(
    payload: systemprompt_models::events::payloads::system::ContextUpdatedPayload,
    message_tx: &UnboundedSender<Message>,
) {
    debug!(context_id = %payload.context_id, "System ContextUpdated");
    send_lifecycle(
        message_tx,
        ContextLifecycleEvent::Updated {
            context_id: payload.context_id,
        },
    );
}

fn handle_context_deleted(
    payload: systemprompt_models::events::payloads::system::ContextDeletedPayload,
    message_tx: &UnboundedSender<Message>,
) {
    debug!(context_id = %payload.context_id, "System ContextDeleted");
    send_lifecycle(
        message_tx,
        ContextLifecycleEvent::Deleted {
            context_id: payload.context_id,
        },
    );
}

fn send_lifecycle(message_tx: &UnboundedSender<Message>, event: ContextLifecycleEvent) {
    let _ = message_tx.send(Message::ContextLifecycle(event));
}
