//! Terminal stream-event handlers: task completion and failure.
//!
//! [`handle_complete`] persists the finished task and broadcasts the success
//! events; [`handle_error`] records the failure. [`send_a2a_status_event`] is
//! the shared helper for emitting an A2A `TaskStatusUpdate` over the SSE
//! channel.

mod complete;
mod error;
mod success;

pub(in crate::services::a2a_server::streaming) use complete::{
    HandleCompleteParams, handle_complete,
};
pub(in crate::services::a2a_server::streaming) use error::{HandleErrorParams, handle_error};

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, TaskId};
use tokio::sync::mpsc::Sender;

use crate::models::a2a::TaskStatus;
use crate::models::a2a::protocol::TaskStatusUpdateEvent;

pub(super) fn send_a2a_status_event(
    tx: &Sender<Event>,
    task_id: &TaskId,
    context_id: &ContextId,
    status: TaskStatus,
    is_final: bool,
) {
    let event = TaskStatusUpdateEvent::new(task_id.clone(), context_id.clone(), status, is_final);
    let jsonrpc = event.to_jsonrpc_response();
    if tx
        .try_send(Event::default().data(jsonrpc.to_string()))
        .is_err()
    {
        tracing::trace!("Failed to send status event, channel closed");
    }
}
