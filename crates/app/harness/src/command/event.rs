//! The event stream of a headless run: one JSON object per line.
//!
//! Every line carries a `type`. `started` opens the stream and `finished`
//! closes it with the exit status; between them are the store's committed
//! writes (`session`, `message`, `part`, `todos` and the two removals), the
//! engine's run status, every permission request and question with the answer
//! the run gave, session errors, and `lagged` when a feed dropped events.
//! Streaming text deltas are not written: a `part` line carries the part as
//! committed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, Write};
use std::path::PathBuf;

use serde::Serialize;
use systemprompt_engine::run::{
    EngineEvent, ErrorKind, PermissionAsk, QuestionAsk, Reply, RunStatus,
};
use systemprompt_engine::store::{
    Change, Message, MessageId, Part, PartId, Session, SessionId, Todo,
};
use systemprompt_identifiers::engine::{PermissionId, QuestionId};

use crate::command::exit::ExitStatus;
use crate::command::permission::{AnswerSource, PermissionMode, QuestionAnswers, UnusedAnswers};

/// Which feed dropped events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Feed {
    Store,
    Engine,
}

/// One line of a headless run's event stream.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    Started {
        session_id: SessionId,
        directory: PathBuf,
        permission_mode: PermissionMode,
        product: String,
        version: String,
    },
    Session {
        session: Box<Session>,
    },
    Status {
        session_id: SessionId,
        status: RunStatus,
    },
    Message {
        message: Box<Message>,
    },
    MessageRemoved {
        session_id: SessionId,
        message_id: MessageId,
    },
    Part {
        part: Box<Part>,
    },
    PartRemoved {
        session_id: SessionId,
        message_id: MessageId,
        part_id: PartId,
    },
    Todos {
        session_id: SessionId,
        todos: Vec<Todo>,
    },
    PermissionAsked {
        ask: Box<PermissionAsk>,
    },
    PermissionReplied {
        id: PermissionId,
        session_id: SessionId,
        reply: Reply,
        source: AnswerSource,
    },
    QuestionAsked {
        ask: QuestionAsk,
    },
    QuestionAnswered {
        id: QuestionId,
        session_id: SessionId,
        answers: QuestionAnswers,
        source: AnswerSource,
    },
    Error {
        session_id: SessionId,
        kind: ErrorKind,
        message: String,
    },
    Lagged {
        feed: Feed,
        missed: u64,
    },
    Finished {
        session_id: Option<SessionId>,
        outcome: ExitStatus,
        exit_code: u8,
        message: Option<String>,
        unused_answers: UnusedAnswers,
    },
}

impl RunEvent {
    pub fn of_change(change: Change) -> Option<Self> {
        match change {
            Change::SessionCreated(session) | Change::SessionUpdated(session) => {
                Some(Self::Session { session })
            },
            Change::MessageUpdated(message) => Some(Self::Message {
                message: Box::new(message),
            }),
            Change::MessageRemoved {
                session_id,
                message_id,
            } => Some(Self::MessageRemoved {
                session_id,
                message_id,
            }),
            Change::PartUpdated(part) => Some(Self::Part {
                part: Box::new(part),
            }),
            Change::PartRemoved {
                session_id,
                message_id,
                part_id,
            } => Some(Self::PartRemoved {
                session_id,
                message_id,
                part_id,
            }),
            Change::TodosUpdated { session_id, todos } => Some(Self::Todos { session_id, todos }),
            Change::ProjectUpdated(_)
            | Change::ProjectDeleted { .. }
            | Change::SessionDeleted { .. }
            | Change::PartDelta { .. } => None,
        }
    }

    pub fn of_engine(event: EngineEvent) -> Option<Self> {
        match event {
            EngineEvent::Status { session, status } => Some(Self::Status {
                session_id: session,
                status,
            }),
            EngineEvent::PermissionAsked(ask) => Some(Self::PermissionAsked { ask }),
            EngineEvent::QuestionAsked(ask) => Some(Self::QuestionAsked { ask }),
            EngineEvent::SessionError { session, error } => Some(Self::Error {
                session_id: session,
                kind: error.kind,
                message: error.message,
            }),
            EngineEvent::PermissionResolved { .. }
            | EngineEvent::QuestionResolved { .. }
            | EngineEvent::McpChanged { .. }
            | EngineEvent::ModelsChanged => None,
        }
    }

    pub fn encode(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Writes events as JSON lines, flushing after each so a reader sees a line as
/// soon as it is written.
#[derive(Debug)]
pub struct EventWriter<W: Write> {
    out: W,
}

impl<W: Write> EventWriter<W> {
    pub const fn new(out: W) -> Self {
        Self { out }
    }

    pub fn emit(&mut self, event: &RunEvent) -> io::Result<()> {
        let mut line = serde_json::to_vec(event).map_err(io::Error::other)?;
        line.push(b'\n');
        self.out.write_all(&line)?;
        self.out.flush()
    }

    pub fn into_inner(self) -> W {
        self.out
    }
}
