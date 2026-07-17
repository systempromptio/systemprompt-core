//! Service layer for the agent crate.
//!
//! Submodules group runtime services by responsibility: the embedded A2A HTTP
//! server, orchestration of agent processes, on-disk agent config authoring,
//! MCP tool bridging, registry, disk-backed skills, message and context
//! services, and shared helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod a2a_server;
pub mod agent_orchestration;
pub mod artifact_publishing;
pub mod config_authoring;
pub mod context;
pub mod context_provider;
pub mod execution_tracking;
pub mod external_integrations;
pub mod mcp;
pub mod message;
pub mod registry;
pub mod registry_provider;
pub mod shared;
pub mod skills;

pub use a2a_server::{AgentHandlerState, Server as AgentServer};

pub use agent_orchestration::{
    AgentEvent, AgentEventBus, AgentOrchestrator, AgentStatus, OrchestrationError,
    OrchestrationResult,
};

pub use registry::AgentRegistry;

pub use config_authoring::{
    AgentConfigAuthoringService, AgentCreateRequest, AgentEditRequest, ConfigAuthoringError,
};

pub use external_integrations::{
    IntegrationError, IntegrationResult, RegisteredMcpServer, ToolExecutionResult, WebhookEndpoint,
    WebhookService,
};

pub use skills::{SkillMetadata, SkillService};

pub use artifact_publishing::ArtifactPublishingService;

pub use artifact_publishing::PublishFromMcpParams;
pub use message::{
    CreateToolExecutionMessageParams, MessageService, PersistMessageInTxParams,
    PersistMessagesParams,
};

pub use context::ContextService;

pub use context_provider::ContextProviderService;

pub use registry_provider::AgentRegistryProviderService;

pub use execution_tracking::ExecutionTrackingService;

pub use shared::{generate_slug, generate_unique_slug};
