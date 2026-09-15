//! Domain models for the agent crate.
//!
//! Submodules:
//! - [`a2a`] — A2A JSON-RPC protocol types (requests, responses, tasks,
//!   artifacts)
//! - [`agent_info`] — lightweight directory listing record
//! - [`context`] — conversational contexts and per-user views
//! - [`runtime`] — runtime metadata describing a live agent process
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod a2a;
pub mod agent_info;
pub mod context;
pub mod database_rows;
pub mod runtime;

pub use a2a::{
    AgentCapabilities, AgentCard, AgentSkill, Artifact, DataPart, Message, Part, Task, TaskState,
    TaskStatus, TextPart, TransportProtocol,
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
