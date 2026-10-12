//! A2A protocol message types: streaming task-update events and the JSON-RPC
//! request/response set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod events;
mod requests;

pub use events::{
    SendMessageResult, StreamResponse, TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};
pub use requests::{
    A2aJsonRpcRequest, A2aParseError, A2aRequest, A2aRequestParams, A2aResponse, CancelTaskParams,
    CancelTaskResponse, GetExtendedAgentCardParams, GetExtendedAgentCardResponse, GetTaskParams,
    GetTaskResponse, ListTasksParams, ListTasksResponse, ListTasksResult, SendMessageConfiguration,
    SendMessageParams, SendMessageResponse, SubscribeToTaskParams, TaskNotCancelableError,
    TaskNotFoundError, UnsupportedOperationError,
};
