//! Stream-setup orchestration: detect the agent kind, validate the context,
//! persist the initial task, register a push-notification config, and assemble
//! a [`StreamSetupResult`] for the streaming event loop.

use std::sync::Arc;

use axum::response::sse::Event;
use serde_json::json;
use systemprompt_identifiers::{AgentName, MessageId, TaskId};
use systemprompt_models::RequestContext;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::models::a2a::Message;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::services::a2a_server::handlers::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageProcessor;

use super::agent_loader::load_agent_runtime;
use super::broadcast::{BroadcastTaskCreatedParams, broadcast_task_created};
use super::initialization_steps::{
    persist_initial_task, save_push_notification_config, validate_context,
};
use super::types::{PersistTaskInput, StreamInput, StreamSetupResult};

pub fn create_jsonrpc_error_event(code: i32, message: &str, request_id: &NumberOrString) -> Event {
    let error_event = json!({
        "jsonrpc": "2.0",
        "error": { "code": code, "message": message },
        "id": request_id
    });
    Event::default().data(error_event.to_string())
}

pub fn detect_mcp_server_and_update_context(
    agent_name: &str,
    context: &mut RequestContext,
    state: &Arc<AgentHandlerState>,
) {
    let is_mcp_server = state
        .agent_state
        .mcp_service_provider()
        .is_some_and(|provider| {
            provider
                .validate_registry()
                .ok()
                .and_then(|()| {
                    provider
                        .find_server(agent_name)
                        .map_err(|e| {
                            tracing::trace!(agent_name = %agent_name, error = %e, "MCP server lookup failed");
                            e
                        })
                        .ok()
                        .flatten()
                })
                .is_some()
        });

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

        context.execution.agent_name = AgentName::new(agent_name.to_string());
    }
}

pub fn resolve_task_id(message: &Message) -> TaskId {
    message
        .task_id
        .clone()
        .unwrap_or_else(|| TaskId::new(Uuid::new_v4().to_string()))
}

pub async fn setup_stream(input: StreamInput, tx: &Sender<Event>) -> Result<StreamSetupResult, ()> {
    let StreamInput {
        message,
        agent_name,
        state,
        request_id,
        mut context,
        callback_config,
    } = input;

    detect_mcp_server_and_update_context(&agent_name, &mut context, &state);

    let task_id = resolve_task_id(&message);
    let context_id = message.context_id.clone();
    let message_id = MessageId::new(Uuid::new_v4().to_string());

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

    broadcast_task_created(BroadcastTaskCreatedParams {
        task_id: &task_id,
        context_id: &context_id,
        user_id: context.user_id().as_str(),
        user_message: &message,
        agent_name: &agent_name,
        token: context.auth.auth_token.as_str(),
    })
    .await;

    save_push_notification_config(&task_id, callback_config.as_ref(), &state).await;

    let agent_runtime =
        load_agent_runtime(&agent_name, &task_id, &task_repo, tx, &request_id).await?;

    let processor =
        MessageProcessor::new(&state.db_pool, Arc::clone(&state.ai_service)).map_err(|e| {
            tracing::error!(error = %e, "Failed to create MessageProcessor");
            if tx
                .try_send(create_jsonrpc_error_event(
                    -32603,
                    &format!("Failed to initialize message processor: {e}"),
                    &request_id,
                ))
                .is_err()
            {
                tracing::trace!("Failed to send error event, channel closed");
            }
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
