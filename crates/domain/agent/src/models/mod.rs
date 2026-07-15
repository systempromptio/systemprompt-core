//! Domain models for the agent crate.
//!
//! Submodules:
//! - [`a2a`] — A2A JSON-RPC protocol types (requests, responses, tasks,
//!   artifacts)
//! - [`agent_info`] — lightweight directory listing record
//! - [`context`] — conversational contexts and per-user views
//! - [`external_integrations`] — descriptors for downstream MCP / OAuth
//!   integrations
//! - [`runtime`] — runtime metadata describing a live agent process
//! - [`web`] — request/response DTOs for the HTTP admin surface

pub mod a2a;
pub mod agent_info;
pub mod context;
pub mod database_rows;
pub mod external_integrations;
pub mod runtime;
pub mod web;

pub use a2a::{
    AgentAuthentication, AgentCapabilities, AgentCard, AgentSkill, Artifact, DataPart, Message,
    Part, Task, TaskState, TaskStatus, TextPart, TransportProtocol,
};

pub use agent_info::AgentInfo;

pub use runtime::AgentRuntimeInfo;

pub use context::{
    ContextDetail, ContextKind, ContextMessage, CreateContextRequest, UpdateContextRequest,
    UserContext, UserContextWithStats,
};

pub use systemprompt_models::{
    ExecutionStep, PlannedTool, StepContent, StepId, StepStatus, StepType, TrackedStep,
};

pub(crate) use database_rows::TaskRow;
pub use database_rows::{
    ArtifactPartRow, ArtifactRow, ExecutionStepBatchRow, MessagePart, TaskMessage,
};

pub use web::*;
