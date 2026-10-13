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

pub use systemprompt_models::a2a::{
    AgentCapabilities, AgentCard, AgentCardBuilder, AgentCardSignature, AgentExtension,
    AgentInterface, AgentProvider, AgentSkill, ApiKeyLocation, ApiKeySecurityScheme, Artifact,
    ArtifactMetadata, AuthorizationCodeOAuthFlow, ClientCredentialsOAuthFlow, DeviceCodeOAuthFlow,
    HttpAuthSecurityScheme, McpServerMetadata, McpToolsParams, Message, MessageMetadata,
    MessageRole, MutualTlsSecurityScheme, OAuth2SecurityScheme, OAuthFlows,
    OpenIdConnectSecurityScheme, PROTOCOL_VERSION, Part, PartContent, ProtocolBinding,
    SecurityRequirement, SecurityScheme, StringList, Task, TaskState, TaskStatus, VERSION_HEADER,
};

pub use protocol::{
    A2aJsonRpcRequest, A2aParseError, A2aRequest, A2aRequestParams, A2aResponse, CancelTaskParams,
    GetExtendedAgentCardParams, GetTaskParams, ListTasksParams, ListTasksResult,
    SendMessageConfiguration, SendMessageParams, SendMessageResult, StreamResponse,
    SubscribeToTaskParams, TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};
