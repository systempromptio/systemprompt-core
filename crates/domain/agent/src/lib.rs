//! # systemprompt-agent
//!
//! Agent-to-Agent (A2A) protocol implementation for systemprompt.io.
//!
//! This crate hosts the domain logic for hosting governed AI agents:
//! - JSON-RPC protocol models in [`models::a2a`]
//! - Persistence in [`repository`] (tasks, contexts, artifacts, execution
//!   steps)
//! - Runtime services in [`services`] (HTTP server, orchestration, MCP
//!   bridging, skills ingestion, streaming, registry)
//! - A typed error hierarchy rooted at [`AgentError`]
//!
//! ## Feature flags
//!
//! This crate currently has no Cargo features beyond defaults. Functionality
//! is unconditional and the crate compiles a single configuration. The facade
//! crate `systemprompt` gates inclusion via the `agent`/`full` features.
//!
//! ## Layer
//!
//! Domain layer. Depends only on `shared/*` and `infra/*` crates plus a small
//! number of sibling domain crates (declared in `Cargo.toml`).

pub(crate) mod error;
pub(crate) mod extension;
pub mod models;
pub mod repository;
pub mod services;
pub(crate) mod state;

/// Compile-time-registered extension entry point for the agent crate.
pub use extension::AgentExtension;

/// Shared runtime state injected into agent HTTP handlers.
pub use state::AgentState;

pub use models::a2a::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, AgentCapabilities, AgentCard, AgentInterface,
    AgentProvider, AgentSkill, Artifact, DataPart, Message, MessageSendParams, Part,
    SecurityScheme, Task, TaskIdParams, TaskQueryParams, TaskState, TaskStatus, TextPart,
    TransportProtocol,
};

pub use error::{
    AgentError, AgentResult, ArtifactError, ContextError, ProtocolError, RowParseError, TaskError,
};

/// A2A protocol version implemented by this crate.
pub const A2A_PROTOCOL_VERSION: &str = "0.3.0";

pub use services::{
    AgentEvent, AgentEventBus, AgentHandlerState, AgentOrchestrator, AgentServer, AgentStatus,
    ContextService, SkillIngestionService, SkillService,
};

pub use repository::content::ArtifactRepository;
