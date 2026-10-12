//! A2A protocol models — agent cards, messages, tasks, artifacts, and
//! transport descriptors. Protocol types retain raw `String` fields to
//! match the external JSON-RPC specification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_card;
pub mod artifact;
pub mod artifact_metadata;
pub mod artifact_summary;
pub mod mcp_extension;
pub mod message;
pub mod methods;
pub mod security;
pub mod stream;
pub mod task;
pub mod task_metadata;
pub mod transport;

pub use agent_card::{
    ARTIFACT_RENDERING_URI, AgentCapabilities, AgentCard, AgentCardBuilder, AgentCardSignature,
    AgentExtension, AgentInterface, AgentProvider, AgentSkill,
};
pub use artifact::Artifact;
pub use artifact_metadata::ArtifactMetadata;
pub use artifact_summary::ArtifactSummary;
pub use mcp_extension::{McpServerMetadata, McpToolsParams, MessageMetadata};
pub use message::{Message, MessageRole, Part, PartContent, PartContentError};
pub use security::{
    ApiKeyLocation, ApiKeySecurityScheme, AuthorizationCodeOAuthFlow, ClientCredentialsOAuthFlow,
    DeviceCodeOAuthFlow, HttpAuthSecurityScheme, MutualTlsSecurityScheme, OAuth2SecurityScheme,
    OAuthFlows, OpenIdConnectSecurityScheme, SecurityRequirement, SecurityScheme, StringList,
};
pub use stream::{
    SendMessageResult, StreamResponse, TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};
pub use task::{Task, TaskState, TaskStatus};
pub use task_metadata::{TaskMetadata, TaskType, agent_names};
pub use transport::ProtocolBinding;

pub const PROTOCOL_VERSION: &str = "1.0";
pub const VERSION_HEADER: &str = "A2A-Version";
