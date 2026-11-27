//! The typed session model: what the engine writes and the shell renders.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod message;
mod part;
mod session;
mod todo;
mod usage;

use serde_json::Value;

pub use message::{
    AssistantMessage, Message, MessageError, MessageErrorKind, MessagePage, MessageWithParts, Role,
    UserMessage,
};
pub use part::{Attachment, Part, PartBody, ToolState};
pub use session::{DiffSummary, NewSession, Project, RevertMarker, Session};
pub use todo::{Todo, TodoPriority, TodoStatus};
pub use usage::{FinishReason, Usage};

pub type JsonMap = serde_json::Map<String, Value>;
