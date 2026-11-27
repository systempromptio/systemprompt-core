//! The contract between the engine and a tool: the trait, the per-call context
//! and the hooks a tool calls back into.
//!
//! The engine holds tools, permission gates and file observers as `dyn` trait
//! objects, which is why the async traits here use `async_trait`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::engine::{CallId, MessageId, SessionId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio_util::sync::CancellationToken;

use crate::tools::error::ToolError;

/// A capability the model can call, held by the engine as `dyn Tool`.
///
/// Implementations are stateless per call; everything call-specific is in the
/// [`ToolContext`]. `schema` is the JSON Schema of the input object; `run`
/// receives input that has not been validated, and each tool validates it
/// against its own schema.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> String;
    // JSON: a tool's input schema is JSON Schema, sent to the model as it is.
    fn schema(&self) -> Value;
    // JSON: tool input is the object the model wrote, checked against the tool's
    // schema.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError>;
}

/// The result of a successful call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub title: String,
    pub output: String,
    // JSON: per-tool metadata the engine stores with the call and the shell renders.
    pub metadata: Value,
    pub attachments: Vec<Attachment>,
}

impl ToolOutput {
    pub fn new(title: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            output: output.into(),
            metadata: Value::Object(Map::new()),
            attachments: Vec::new(),
        }
    }

    // JSON: per-tool metadata the engine stores with the call and the shell
    // renders.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    #[must_use]
    pub fn with_attachment(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }
}

/// Binary content returned alongside the text output, such as an image the
/// model should see. `data` is the standard base64 of the bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub mime: String,
    pub filename: Option<String>,
    pub data: String,
}

impl Attachment {
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime, self.data)
    }
}

/// One question to the permission layer.
///
/// `permission` names it (`shell`, `edit`, `read`,
/// `external_directory`, …); every pattern in `patterns` must be allowed;
/// `always` holds the patterns remembered when the user approves "always",
/// where a trailing ` *` means "with any arguments".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub permission: String,
    pub patterns: Vec<String>,
    pub always: Vec<String>,
    pub title: String,
    // JSON: details shown with the question; their shape depends on the permission.
    pub metadata: Value,
}

/// Decides whether a tool may go ahead, held as `dyn PermissionGate` in every
/// [`ToolContext`].
///
/// Implemented by the engine on top of its rules and the user: `ask` resolves
/// when allowed, and fails with `Denied` for a rule or `Rejected` when the user
/// said no.
#[async_trait]
pub trait PermissionGate: Send + Sync {
    async fn ask(&self, request: PermissionRequest) -> Result<(), ToolError>;
}

/// Told about every file a tool writes, held as `dyn FileObserver`.
///
/// `before_write` runs before a file is created, changed or removed (for
/// snapshots), `after_write` after it, and the text it returns (diagnostics, a
/// formatter's note) is appended to the tool output.
#[async_trait]
pub trait FileObserver: Send + Sync {
    async fn before_write(&self, path: &Path);
    async fn after_write(&self, path: &Path) -> Option<String>;
}

/// Receives live updates for a running call, such as streamed shell output.
pub trait Progress: Send + Sync {
    // JSON: live metadata of a running call, rendered by the shell as it arrives.
    fn update(&self, title: &str, metadata: Value);
}

/// Everything one tool call knows about where and for whom it runs.
///
/// Relative paths resolve
/// against `cwd`; anything outside `worktree` asks `external_directory` first;
/// `data_dir` is where tools keep files of their own (spilled output,
/// background logs).
#[derive(Clone)]
pub struct ToolContext {
    pub session_id: SessionId,
    pub message_id: MessageId,
    pub call_id: CallId,
    pub agent: String,
    pub cwd: PathBuf,
    pub worktree: PathBuf,
    pub data_dir: PathBuf,
    pub cancel: CancellationToken,
    pub gate: Arc<dyn PermissionGate>,
    pub observer: Option<Arc<dyn FileObserver>>,
    pub progress: Option<Arc<dyn Progress>>,
}

impl fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolContext")
            .field("session_id", &self.session_id)
            .field("message_id", &self.message_id)
            .field("call_id", &self.call_id)
            .field("agent", &self.agent)
            .field("cwd", &self.cwd)
            .field("worktree", &self.worktree)
            .field("data_dir", &self.data_dir)
            .field("cancelled", &self.cancel.is_cancelled())
            .field("observer", &self.observer.is_some())
            .field("progress", &self.progress.is_some())
            .finish_non_exhaustive()
    }
}

impl ToolContext {
    pub async fn ask(&self, request: PermissionRequest) -> Result<(), ToolError> {
        self.gate.ask(request).await
    }

    // JSON: live metadata of a running call, rendered by the shell as it arrives.
    pub fn report(&self, title: &str, metadata: Value) {
        if let Some(progress) = &self.progress {
            progress.update(title, metadata);
        }
    }

    pub fn ensure_live(&self) -> Result<(), ToolError> {
        if self.cancel.is_cancelled() {
            Err(ToolError::Aborted)
        } else {
            Ok(())
        }
    }
}
