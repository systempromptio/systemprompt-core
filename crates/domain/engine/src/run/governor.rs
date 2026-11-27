//! The Governor contract: the only source of models, the signer of every model
//! call, the voice asked before every tool and the source of the extensions an
//! instance enabled. The engine cannot reach a model without one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error as StdError;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::llm::{Endpoint, ModelSpec, Usage};
use crate::mcp::HeaderSource;
use serde_json::Value;
use systemprompt_identifiers::ModelId;
use systemprompt_identifiers::engine::{CallId, MessageId, SessionId};

use crate::run::extension::EnabledExtensions;

/// Supplies models, MCP servers and the enabled extensions, authorises every
/// model call and gives its verdict on every tool call.
///
/// The engine holds it as `dyn Governor`, which is why the trait uses
/// `async_trait`. Subagent sessions go through the same calls as the sessions
/// the user started.
///
/// The model list is empty while no instance is applied, and a managed MCP
/// server replaces the user's server of the same name. An authorisation error
/// fails the step before anything is sent; after the response head,
/// `ResponseAction::Retry` authorises and resends the step once. The
/// Governor gives its verdict on a tool after the permission rules allowed it
/// and before it runs; a denial's reason goes back to the model, and no
/// extension can turn it into an allow. `extensions` is the applied
/// instance's enablement: `None`, or an id missing from the set, withholds
/// that extension, and only required extensions contribute regardless. The
/// generation bumps when models or MCP servers changed, and the engine then
/// re-reads both. `governed_by` names the organisation whose policy reviews
/// tool calls in the system prompt. What the Governor only needs to hear (new
/// user messages, finished tool calls, session lifecycle) reaches it as an
/// extension's [`GovernorObserver`](crate::run::extension::GovernorObserver).
#[async_trait::async_trait]
pub trait Governor: Send + Sync {
    fn models(&self) -> Vec<ModelSpec>;
    fn mcp_servers(&self) -> Vec<McpSpec>;
    async fn authorize(&self, call: &ModelCall) -> Result<Endpoint, GovernError>;
    async fn response(&self, call: &ModelCall, status: u16, attempt: u32) -> ResponseAction;
    async fn step_finished(&self, call: &ModelCall, usage: &Usage);
    async fn tool_before(&self, call: &ToolCall) -> Verdict;
    fn extensions(&self) -> Option<EnabledExtensions>;
    fn generation(&self) -> u64;
    fn governed_by(&self) -> Option<String> {
        None
    }
}

/// A remote MCP server the instance manages. Its headers are read again before
/// every request.
#[derive(Clone)]
pub struct McpSpec {
    pub name: String,
    pub url: String,
    pub headers: Arc<dyn HeaderSource>,
}

impl fmt::Debug for McpSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpSpec")
            .field("name", &self.name)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

/// Why a model call is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Purpose {
    Step,
    Title,
    Compaction,
    Summary,
}

/// One model call, as the Governor sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCall {
    pub session_id: SessionId,
    pub model_id: ModelId,
    pub purpose: Purpose,
    pub directory: PathBuf,
}

/// What to do after the response head of a model call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseAction {
    Continue,
    Retry,
    Fail(String),
}

/// A new user message, before its first step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPrompt {
    pub session_id: SessionId,
    pub message_id: MessageId,
    pub text: String,
    pub agent: String,
    pub model_id: ModelId,
    pub directory: PathBuf,
}

/// A tool call the permission rules allowed. `skill` names the skill a `skill`
/// call loads.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub session_id: SessionId,
    pub call_id: CallId,
    pub tool: String,
    // JSON: the tool input as the model wrote it, repaired and checked against the tool's schema.
    pub input: Value,
    pub directory: PathBuf,
    pub skill: Option<String>,
}

/// How a tool call ended; `error` is set when it failed.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutcome {
    pub title: String,
    pub output: String,
    // JSON: the tool's own metadata, shaped by each tool.
    pub metadata: Value,
    pub error: Option<String>,
}

/// The policy decision on one tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny { reason: String },
}

/// Session lifecycle moments observers are told about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Created {
        session_id: SessionId,
        parent_id: Option<SessionId>,
        directory: PathBuf,
    },
    Deleted {
        session_id: SessionId,
    },
    Idle {
        session_id: SessionId,
    },
}

/// Why the Governor would not authorise a model call. Never carries a
/// credential.
#[derive(Debug, thiserror::Error)]
pub enum GovernError {
    #[error("not signed in to an instance")]
    NotSignedIn,
    #[error("model {model} is not offered by the instance")]
    ModelNotOffered { model: String },
    #[error("the instance refused the call: {reason}")]
    Refused { reason: String },
    #[error("the instance could not be reached")]
    Unavailable {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}
