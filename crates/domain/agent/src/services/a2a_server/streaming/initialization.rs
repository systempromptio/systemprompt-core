use std::sync::Arc;

use axum::response::sse::Event;
use serde_json::json;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::{RequestContext, TaskMetadata};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::protocol::PushNotificationConfig;
use crate::models::a2a::{Message, Task, TaskState, TaskStatus};
use crate::models::AgentRuntimeInfo;
use crate::repository::content::PushNotificationConfigRepository;
use crate::repository::context::ContextRepository;
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::errors::classify_database_error;
use crate::services::a2a_server::handlers::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageProcessor;

use super::agent_loader::load_agent_runtime;
use super::broadcast::broadcast_task_created;

pub struct StreamInput {
    pub message: Message,
    pub agent_name: String,
    pub state: Arc<AgentHandlerState>,
    pub request_id: NumberOrString,
    pub context: RequestContext,
    pub callback_config: Option<PushNotificationConfig>,
}

pub struct StreamSetupResult {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub message_id: String,
    pub message: Message,
    pub agent_name: String,
    pub context: RequestContext,
    pub task_repo: TaskRepository,
    pub agent_runtime: AgentRuntimeInfo,
    pub processor: Arc<MessageProcessor>,
    pub request_id: NumberOrString,
}

pub fn create_jsonrpc_error_event(code: i32, message: &str, request_id: &NumberOrString) -> Event {
    let error_event = json!({
        "jsonrpc": "2.0",
        "error": { "code": code, "message": message },
        "id": request_id
    });
    Event::default().data(error_event.to_string())
}

pub async fn detect_mcp_server_and_update_context(agent_name: &str, context: &mut RequestContext) {
    use systemprompt_mcp::services::registry::McpServerRegistry;

    let is_mcp_server = match McpServerRegistry::validate() {
        Ok(()) => McpServerRegistry::find_server(agent_name)
            .ok()
            .flatten()
            .is_some(),
        Err(_) => false,
    };

    if is_mcp_server && context.agent_name().as_str() != agent_name {
        tracing::info!(
            agent_name = %agent_name,
            context_agent = %context.agent_name().as_str(),
            "MCP server handling request from agent"
        );
    } else if !is_mcp_server && context.agent_name().as_str() != agent_name {
        tracing::warn!(
            context_agent = %context.agent_name().as_str(),
            service_agent = %agent_name,
            "Agent mismatch, using service name"
        );

        use systemprompt_identifiers::AgentName;
        context.execution.agent_name = AgentName::new(agent_name.to_string());
    }
}

pub fn resolve_task_id(message: &Message) -> TaskId {
    message
        .task_id
        .clone()
        .unwrap_or_else(|| TaskId::new(Uuid::new_v4().to_string()))
}

pub async fn validate_context(
    context_id: &ContextId,
    user_id: &UserId,
    state: &Arc<AgentHandlerState>,
    tx: &UnboundedSender<Event>,
    request_id: &NumberOrString,
) -> Result<(), ()> {
    let context_repo = ContextRepository::new(state.db_pool.clone());

    context_repo
        .get_context(context_id, user_id)
        .await
        .map_err(|e| {
            tracing::error!(
                context_id = %context_id,
                user_id = %user_id,
                error = %e,
                "Context validation failed"
            );
            let _ = tx.send(create_jsonrpc_error_event(
                -32603,
                &format!("Context validation failed: {e}"),
                request_id,
            ));
        })?;

    tracing::info!(
        context_id = %context_id,
        user_id = %user_id,
        "Context validated"
    );

    Ok(())
}

pub struct PersistTaskInput<'a> {
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub agent_name: &'a str,
    pub context: &'a RequestContext,
    pub state: &'a Arc<AgentHandlerState>,
    pub tx: &'a UnboundedSender<Event>,
    pub request_id: &'a NumberOrString,
}

pub async fn persist_initial_task(input: PersistTaskInput<'_>) -> Result<TaskRepository, ()> {
    let PersistTaskInput {
        task_id,
        context_id,
        agent_name,
        context,
        state,
        tx,
        request_id,
    } = input;

    let task_repo = TaskRepository::new(state.db_pool.clone());
    let metadata = TaskMetadata::new_agent_message(agent_name.to_string());

    let task = Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: Some(metadata),
        kind: "task".to_string(),
    };

    task_repo
        .create_task(
            &task,
            &UserId::new(context.user_id().as_str()),
            &SessionId::new(context.session_id().as_str()),
            &TraceId::new(context.trace_id().as_str()),
            agent_name,
        )
        .await
        .map_err(|e| {
            tracing::error!(task_id = %task_id, error = %e, "Failed to persist task at start");
            let error_detail = classify_database_error(&e);
            let _ = tx.send(create_jsonrpc_error_event(
                -32603,
                &format!("Failed to create task: {error_detail}"),
                request_id,
            ));
        })?;

    tracing::info!(task_id = %task_id, "Task persisted to database at stream start");

    if let Err(e) = task_repo
        .track_agent_in_context(context_id, agent_name)
        .await
    {
        tracing::warn!(context_id = %context_id, error = %e, "Failed to track agent in context");
    }

    Ok(task_repo)
}

pub async fn save_push_notification_config(
    task_id: &TaskId,
    callback_config: &Option<PushNotificationConfig>,
    state: &Arc<AgentHandlerState>,
) {
    let Some(ref config) = callback_config else {
        return;
    };

    tracing::info!(url = %config.url, "Push notification callback registered");

    let config_repo = match PushNotificationConfigRepository::new(state.db_pool.clone()) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to create PushNotificationConfigRepository");
            return;
        },
    };

    match config_repo.add_config(task_id, config).await {
        Ok(_) => tracing::info!(task_id = %task_id, "Push notification config saved"),
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to save push notification config")
        },
    }
}

pub async fn setup_stream(
    input: StreamInput,
    tx: &UnboundedSender<Event>,
) -> Result<StreamSetupResult, ()> {
    let StreamInput {
        message,
        agent_name,
        state,
        request_id,
        mut context,
        callback_config,
    } = input;

    detect_mcp_server_and_update_context(&agent_name, &mut context).await;

    let task_id = resolve_task_id(&message);
    let context_id = message.context_id.clone();
    let message_id = Uuid::new_v4().to_string();

    tracing::info!(
        task_id = %task_id,
        context_id = %context_id,
        message_id = %message_id,
        "Generated IDs"
    );

    validate_context(&context_id, context.user_id(), &state, tx, &request_id).await?;

    let persist_input = PersistTaskInput {
        task_id: &task_id,
        context_id: &context_id,
        agent_name: &agent_name,
        context: &context,
        state: &state,
        tx,
        request_id: &request_id,
    };
    let task_repo = persist_initial_task(persist_input).await?;

    broadcast_task_created(
        &task_id,
        &context_id,
        context.user_id().as_str(),
        &message,
        &agent_name,
        context.auth.auth_token.as_str(),
    )
    .await;

    save_push_notification_config(&task_id, &callback_config, &state).await;

    let agent_runtime =
        load_agent_runtime(&agent_name, &task_id, &task_repo, tx, &request_id).await?;

    let processor = MessageProcessor::new(state.db_pool.clone(), state.ai_service.clone())
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create MessageProcessor");
            let _ = tx.send(create_jsonrpc_error_event(
                -32603,
                &format!("Failed to initialize message processor: {e}"),
                &request_id,
            ));
        })?;

    Ok(StreamSetupResult {
        task_id,
        context_id,
        message_id,
        message,
        agent_name,
        context,
        task_repo,
        agent_runtime,
        processor: Arc::new(processor),
        request_id,
    })
}
