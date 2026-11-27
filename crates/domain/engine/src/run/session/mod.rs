//! Sessions: admission of prompts, the run loop, history mapping, compaction,
//! titles and revert.
//!
//! One run per session at a time. A prompt sent while a run is going is held in
//! the session's inbox and either delivered as a steer at the next step
//! boundary or queued until the run is idle, as the sender chose.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod admit;
pub(crate) mod attachments;
pub(crate) mod commands;
pub(crate) mod compaction;
pub(crate) mod history;
pub(crate) mod inbox;
pub(crate) mod meta;
pub(crate) mod model_call;
pub(crate) mod revert;
pub(crate) mod run;
pub(crate) mod runs;
pub(crate) mod step;
pub(crate) mod stream;
pub(crate) mod subtask;
pub(crate) mod system;
pub(crate) mod title;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::store::{MessageId, NewSession, PartBody, Session, SessionId};
use serde::{Deserialize, Serialize};

pub(crate) use runs::{Admit, Runs};

use crate::run::error::EngineError;
use crate::run::governor::SessionEvent;
use crate::run::inner::{Inner, project_name};

/// How a prompt sent during a run reaches the model: `Steer` delivers it at the
/// next step boundary of the running turn, `Queue` as a new turn once the run
/// is idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    #[default]
    Steer,
    Queue,
}

/// A file to send with a prompt: a path (relative to the prompt's directory) or
/// a base64 data URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentInput {
    Path(PathBuf),
    DataUrl { url: String, name: Option<String> },
}

/// A user prompt. Agent and model fall back to the session's, then the
/// configuration's, then the first model on offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptInput {
    pub session_id: SessionId,
    pub directory: PathBuf,
    pub text: String,
    pub attachments: Vec<AttachmentInput>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub delivery: Delivery,
}

/// What became of a prompt.
///
/// `Started` when the session was idle, so the message is stored and a run
/// started; `Steered` when it is held for the next step boundary of the running
/// turn; `Queued` when it is held until the running turn ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admitted {
    Started { message_id: MessageId },
    Steered,
    Queued,
}

/// A slash command typed in a session, without its leading slash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInput {
    pub session_id: SessionId,
    pub directory: PathBuf,
    pub name: String,
    pub arguments: String,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub delivery: Delivery,
}

/// What a slash command did.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandOutcome {
    Prompted(Admitted),
    Compacting,
    Reverted(Box<Session>),
}

/// A slash command the shell can offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub description: Option<String>,
    pub builtin: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Admission {
    pub(crate) agent: String,
    pub(crate) model: String,
    pub(crate) parts: Vec<PartBody>,
    pub(crate) text: String,
    pub(crate) system: bool,
    pub(crate) delivery: Delivery,
    pub(crate) directory: PathBuf,
}

pub(crate) async fn create(
    inner: &Inner,
    directory: &Path,
    parent: Option<SessionId>,
    title: Option<String>,
    agent: Option<String>,
) -> Result<Session, EngineError> {
    let config = inner.config(directory).await?;
    let root = config.worktree.root.clone();
    let name = project_name(&root);
    let project = inner
        .db
        .run(move |store| store.ensure_project(&root, &name))
        .await?;
    let mut new = NewSession::new(project.id, directory);
    new.parent_id = parent;
    new.title = title.unwrap_or_default();
    new.agent = agent;
    let session = inner.db.run(move |store| store.create_session(new)).await?;
    inner.governed.session_event(&SessionEvent::Created {
        session_id: session.id.clone(),
        parent_id: session.parent_id.clone(),
        directory: session.directory.clone(),
    });
    tracing::debug!(
        session = %session.id,
        directory = %session.directory.display(),
        "session created"
    );
    Ok(session)
}

pub(crate) async fn delete(inner: &Arc<Inner>, id: &SessionId) -> Result<bool, EngineError> {
    let mut tree = vec![id.clone()];
    let mut index = 0;
    while let Some(current) = tree.get(index).cloned() {
        let children = inner
            .db
            .run(move |store| store.child_sessions(&current))
            .await?;
        tree.extend(children.into_iter().map(|child| child.id));
        index += 1;
    }
    for session in &tree {
        inner.runs.abort(session);
        let killed = inner.jobs.kill_all(session);
        if killed > 0 {
            tracing::debug!(session = %session, killed, "background jobs stopped");
        }
        inner.asks.cancel_session(inner, session);
        inner.forget_session(session);
    }
    let key = id.clone();
    let removed = inner
        .db
        .run(move |store| store.delete_session(&key))
        .await?;
    if removed {
        for session in tree {
            inner.governed.session_event(&SessionEvent::Deleted {
                session_id: session,
            });
        }
    }
    Ok(removed)
}
