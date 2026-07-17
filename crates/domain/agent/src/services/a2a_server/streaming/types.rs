//! A2A streaming request/response types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::Event;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::RequestContext;
use tokio::sync::mpsc::Sender;

use crate::models::AgentRuntimeInfo;
use crate::models::a2a::Message;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::protocol::PushNotificationConfig;
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::handlers::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageProcessor;

#[derive(Debug)]
pub struct StreamInput {
    pub message: Message,
    pub agent_name: String,
    pub state: Arc<AgentHandlerState>,
    pub request_id: NumberOrString,
    pub context: RequestContext,
    pub callback_config: Option<PushNotificationConfig>,
}

#[derive(Debug)]
pub struct StreamSetupResult {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub message_id: MessageId,
    pub message: Message,
    pub agent_name: String,
    pub context: RequestContext,
    pub task_repo: TaskRepository,
    pub agent_runtime: AgentRuntimeInfo,
    pub processor: Arc<MessageProcessor>,
    pub request_id: NumberOrString,
}

#[derive(Debug)]
pub struct PersistTaskInput<'a> {
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub agent_name: &'a str,
    pub context: &'a RequestContext,
    pub state: &'a Arc<AgentHandlerState>,
    pub tx: &'a Sender<Event>,
    pub request_id: &'a NumberOrString,
}

#[derive(Debug)]
pub struct StreamContext {
    pub tx: Sender<Event>,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub message_id: MessageId,
    pub request_id: NumberOrString,
    pub task_repo: TaskRepository,
    pub state: Arc<AgentHandlerState>,
    pub processor: Arc<MessageProcessor>,
}
