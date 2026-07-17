//! A2A JSON-RPC protocol types.
//!
//! Agent cards, tasks, messages, artifacts, and request/response envelopes.
//! Core shapes are re-exported from `systemprompt_models`; this module adds the
//! JSON-RPC framing and the request parameter types specific to the agent
//! crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod jsonrpc;
pub mod protocol;
mod service_status;

pub use systemprompt_models::a2a::{
    AgentAuthentication, AgentCapabilities, AgentCard, AgentCardBuilder, AgentCardSignature,
    AgentExtension, AgentInterface, AgentProvider, AgentSkill, ApiKeyLocation, Artifact,
    ArtifactMetadata, DataPart, FileContent, FilePart, McpServerMetadata, McpToolsParams, Message,
    MessageMetadata, MessageRole, OAuth2Flow, OAuth2Flows, Part, SecurityScheme, Task, TaskState,
    TaskStatus, TextPart, TransportProtocol,
};

pub use protocol::{
    A2aJsonRpcRequest, A2aParseError, A2aRequest, A2aRequestParams, A2aResponse, MessageSendParams,
    TaskIdParams, TaskQueryParams,
};
pub use service_status::ServiceStatusParams;
