//! The change feed's events, one per committed write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::id::{MessageId, PartId, ProjectId, SessionId};
use crate::model::{Message, Part, Project, Session, Todo};

/// A committed write, published on [`crate::Store::subscribe`] after its
/// transaction commits.
///
/// Sessions are boxed to keep the enum small. Deleting a session removes its
/// messages, parts and todos with it; only one `SessionDeleted` per session is
/// sent for that.
#[derive(Debug, Clone)]
pub enum Change {
    ProjectUpdated(Project),
    ProjectDeleted {
        id: ProjectId,
    },
    SessionCreated(Box<Session>),
    SessionUpdated(Box<Session>),
    SessionDeleted {
        id: SessionId,
        project_id: ProjectId,
        parent_id: Option<SessionId>,
    },
    MessageUpdated(Message),
    MessageRemoved {
        session_id: SessionId,
        message_id: MessageId,
    },
    PartUpdated(Part),
    PartRemoved {
        session_id: SessionId,
        message_id: MessageId,
        part_id: PartId,
    },
    PartDelta {
        session_id: SessionId,
        message_id: MessageId,
        part_id: PartId,
        delta: String,
    },
    TodosUpdated {
        session_id: SessionId,
        todos: Vec<Todo>,
    },
}

impl Change {
    pub fn session_id(&self) -> Option<&SessionId> {
        match self {
            Self::ProjectUpdated(_) | Self::ProjectDeleted { .. } => None,
            Self::SessionCreated(session) | Self::SessionUpdated(session) => Some(&session.id),
            Self::SessionDeleted { id, .. } => Some(id),
            Self::MessageUpdated(message) => Some(message.session_id()),
            Self::PartUpdated(part) => Some(&part.session_id),
            Self::MessageRemoved { session_id, .. }
            | Self::PartRemoved { session_id, .. }
            | Self::PartDelta { session_id, .. }
            | Self::TodosUpdated { session_id, .. } => Some(session_id),
        }
    }
}
