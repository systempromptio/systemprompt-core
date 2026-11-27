//! The durable session model of the engine, kept in one SQLite database.
//!
//! A [`Store`] holds projects, sessions (with subagent children), messages,
//! their parts and per-session todo lists. Every method is synchronous and
//! short; async callers wrap them in `spawn_blocking`. Each committed write is
//! published on a broadcast change feed ([`Store::subscribe`]) so a UI can
//! render from a single stream instead of polling.
//!
//! Identifiers ([`ProjectId`], [`SessionId`], [`MessageId`], [`PartId`]) are
//! prefixed strings that sort in creation order, which is the order the store
//! returns messages and parts in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod change;
mod error;
mod handle;
mod id;
mod model;
mod rows;
mod schema;

pub use change::Change;
pub use error::{Result, StoreError};
pub use handle::Store;
pub use id::{CallId, IdClock, IdError, MessageId, PartId, ProjectId, SessionId, now_ms};
pub use model::{
    AssistantMessage, Attachment, DiffSummary, FinishReason, JsonMap, Message, MessageError,
    MessageErrorKind, MessagePage, MessageWithParts, NewSession, Part, PartBody, Project,
    RevertMarker, Role, Session, Todo, TodoPriority, TodoStatus, ToolState, Usage, UserMessage,
};
