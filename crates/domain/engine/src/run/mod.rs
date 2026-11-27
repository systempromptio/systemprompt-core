//! The agent engine: sessions, the run loop, tools, permissions, compaction and
//! subagents, governed by construction.
//!
//! An [`Engine`] is built with a [`Governor`]. The Governor is the only source
//! of models, authorises every model call, and gives its verdict before every
//! tool runs; without models on offer a prompt fails with
//! [`ErrorKind::NotSignedIn`]. An [`extension`] adds tools, prompt sections,
//! agents, commands and MCP servers and takes part in governed calls, only
//! while the Governor's enablement admits it. The engine meets every other
//! engine crate here: configuration (agents, rules, skills, commands,
//! instructions), the session store, the model clients, the tools, MCP servers,
//! workspace snapshots, language servers and the prompt texts.
//!
//! The shell drives sessions through [`Engine`]'s methods, listens to
//! [`Engine::subscribe`] for run status, permission requests, questions and
//! failures, and renders messages and parts from the store's own change feed.
//! Every task the engine starts belongs to one task set that
//! [`Engine::shutdown`] drains.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod db;
/// The marker a refused tool call carries in its stored part's metadata.
pub mod denied;
mod directory;
mod engine;
mod error;
mod events;
/// The extension contract: what an extension adds to sessions and how a set
/// of them resolves under an instance's enablement.
pub mod extension;
mod governed;
mod governor;
mod inner;
mod options;
mod session;
mod tools;

pub use engine::Engine;
pub use error::{EngineError, ErrorKind};
pub use events::{
    EngineEvent, PermissionAsk, Question, QuestionAsk, QuestionOption, Reply, RunStatus,
    SessionFailure,
};
pub use extension::{
    CallAugmenter, EnabledExtensions, EngineExtension, ExtensionMetadata, ExtensionSet,
    ExtensionTool, GovernorObserver, ResolveError, ResponseHead, Review, ToolReviewer,
};
pub use governor::{
    GovernError, Governor, McpSpec, ModelCall, NullGovernor, Purpose, ResponseAction, SessionEvent,
    ToolCall, ToolOutcome, UserPrompt, Verdict,
};
pub use harness_ids::{CallId, ExtensionId, PermissionId, QuestionId};
pub use harness_llm::{Endpoint, ModelSpec, Protocol, Usage};
pub use options::EngineOptions;
pub use session::{
    Admitted, AttachmentInput, CommandInfo, CommandInput, CommandOutcome, Delivery, PromptInput,
};
