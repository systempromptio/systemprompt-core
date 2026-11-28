//! The control stream of a run: one JSON command per line, read while the run
//! is in progress.
//!
//! Under `--control` standard input carries [`ControlCommand`]s. `prompt`
//! starts a turn, or queues one behind the running turn; `steer` reaches the
//! running turn at its next step; `abort` stops the run; `revert` takes the
//! session back to one of its own prompts and is honoured only while the
//! session is idle; `reply` answers the permission request with that id, and
//! every request waits for one; `end` says nothing more is coming. A run
//! given a control feed stays open after its prompts until the feed ends, and
//! then finishes once the session is idle. Requests still waiting when the
//! feed ends, and those raised afterwards, are answered as in a run without
//! one. A line that does not decode ends the run as a usage error. The caller
//! decides when to send each command from the event stream it reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, BufRead, Write};

use serde::Deserialize;
use systemprompt_engine::run::{Delivery, Engine, PromptInput, Reply, RunStatus};
use systemprompt_engine::store::{Message, MessageId, MessageWithParts, SessionId};
use systemprompt_identifiers::engine::PermissionId;
use systemprompt_traits::OwnedTask;
use tokio::sync::mpsc;

use crate::command::event::RunEvent;
use crate::command::exit::ExitStatus;
use crate::command::permission::AnswerSource;
use crate::command::pump::{Limits, Pump, PumpError, expired};

const READER: &str = "headless control";

/// The feed a run takes its [`ControlCommand`]s from. A feed that closes is
/// read as `end`.
pub type ControlFeed = mpsc::UnboundedReceiver<Result<ControlCommand, ControlError>>;

/// One instruction to a run in progress; on a line it is a JSON object whose
/// `do` names it, and a key it does not know is refused.
///
/// `revert` names the prompt to revert to by its position among the
/// session's own user messages, steering included, counted from 1: that
/// message and everything after it are removed. `reply` carries the engine's
/// own reply (`{"type":"once"}`, `{"type":"always"}`,
/// `{"type":"reject","feedback":"..."}`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlCommand {
    Prompt { text: String },
    Steer { text: String },
    Abort {},
    Revert { prompt: usize },
    Reply { id: PermissionId, reply: Reply },
    End {},
}

/// Why a control stream stopped a run.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("line {line} of the control stream does not decode")]
    Undecodable {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("the control stream could not be read")]
    Unreadable(#[source] io::Error),
    #[error("the session has no user prompt number {prompt}")]
    NoSuchPrompt { prompt: usize },
}

pub(crate) enum Idle {
    Command(ControlCommand),
    Running,
    Closed,
}

impl ControlCommand {
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

impl ControlError {
    pub const fn status(&self) -> ExitStatus {
        match self {
            Self::Undecodable { .. } | Self::NoSuchPrompt { .. } => ExitStatus::Usage,
            Self::Unreadable(_) => ExitStatus::Internal,
        }
    }
}

pub fn read_controls<R>(input: R) -> (ControlFeed, OwnedTask<()>)
where
    R: BufRead + Send + 'static,
{
    let (sender, feed) = mpsc::unbounded_channel();
    let reader = OwnedTask::spawn_blocking(READER, move || forward(input, &sender));
    (feed, reader)
}

fn forward(
    input: impl BufRead,
    sender: &mpsc::UnboundedSender<Result<ControlCommand, ControlError>>,
) {
    for (index, line) in input.lines().enumerate() {
        let command = match line {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => ControlCommand::parse(&line).map_err(|source| ControlError::Undecodable {
                line: index + 1,
                source,
            }),
            Err(source) => Err(ControlError::Unreadable(source)),
        };
        let last = matches!(command, Err(_) | Ok(ControlCommand::End {}));
        if sender.send(command).is_err() || last {
            return;
        }
    }
}

fn nth_prompt(messages: &[MessageWithParts], prompt: usize) -> Option<MessageId> {
    let index = prompt.checked_sub(1)?;
    messages
        .iter()
        .filter_map(|entry| match &entry.message {
            Message::User(user) if !user.system => Some(user.id.clone()),
            Message::User(_) | Message::Assistant(_) => None,
        })
        .nth(index)
}

pub(crate) async fn next(controls: &mut Option<ControlFeed>) -> Result<ControlCommand, PumpError> {
    match controls {
        Some(feed) => feed
            .recv()
            .await
            .unwrap_or(Ok(ControlCommand::End {}))
            .map_err(PumpError::Control),
        None => std::future::pending().await,
    }
}

async fn send(
    engine: &Engine,
    template: &PromptInput,
    text: String,
    delivery: Delivery,
) -> Result<(), PumpError> {
    let input = PromptInput {
        text,
        delivery,
        ..template.clone()
    };
    let admitted = engine.prompt(input).await;
    admitted.map(|_| ()).map_err(PumpError::Engine)
}

async fn revert(engine: &Engine, root: &SessionId, prompt: usize) -> Result<(), PumpError> {
    let messages = engine.messages(root).await;
    let messages = messages.map_err(PumpError::Engine)?;
    let message_id = nth_prompt(&messages, prompt)
        .ok_or(PumpError::Control(ControlError::NoSuchPrompt { prompt }))?;
    let reverted = engine.revert(root, &message_id).await;
    reverted.map(|_| ()).map_err(PumpError::Engine)
}

impl<W: Write + Send> Pump<'_, W> {
    pub(crate) async fn carry_out(&mut self, command: ControlCommand) -> Result<(), PumpError> {
        match command {
            ControlCommand::Prompt { text } => {
                send(self.engine, &self.template, text, Delivery::Queue).await
            },
            ControlCommand::Steer { text } => {
                send(self.engine, &self.template, text, Delivery::Steer).await
            },
            ControlCommand::Abort {} => {
                self.interrupt(ExitStatus::Aborted);
                Ok(())
            },
            ControlCommand::Revert { prompt } => revert(self.engine, &self.root, prompt).await,
            ControlCommand::Reply { id, reply } => self.reply(&id, reply),
            ControlCommand::End {} => self.close(),
        }
    }

    pub(crate) async fn while_idle(&mut self, limits: &Limits<'_>) -> Result<Idle, PumpError> {
        loop {
            if self.engine.status(&self.root) != RunStatus::Idle {
                return Ok(Idle::Running);
            }
            if self.feeds.controls.is_none() {
                return Ok(Idle::Closed);
            }
            tokio::select! {
                biased;
                change = self.feeds.changes.recv() => self.on_change(change)?,
                event = self.feeds.events.recv() => {
                    self.on_event(event)?;
                },
                command = next(&mut self.feeds.controls) => match command? {
                    ControlCommand::End {} => self.close()?,
                    command => return Ok(Idle::Command(command)),
                },
                () = limits.cancel.cancelled() => {
                    self.interrupted = Some(ExitStatus::Aborted);
                    return Ok(Idle::Closed);
                },
                () = expired(limits.deadline) => {
                    self.interrupted = Some(ExitStatus::TimedOut);
                    return Ok(Idle::Closed);
                },
            }
        }
    }

    fn reply(&mut self, id: &PermissionId, reply: Reply) -> Result<(), PumpError> {
        let held = self
            .waiting
            .as_mut()
            .and_then(|waiting| waiting.release(id));
        let Some(session_id) = held else {
            tracing::warn!(permission = %id, "no permission request waits under that id");
            return Ok(());
        };
        if !self.engine.reply_permission(id, reply.clone()) {
            return Ok(());
        }
        self.emit(&RunEvent::PermissionReplied {
            id: id.clone(),
            session_id,
            reply,
            source: AnswerSource::Control,
        })
    }

    fn close(&mut self) -> Result<(), PumpError> {
        self.feeds.controls = None;
        let Some(mut waiting) = self.waiting.take() else {
            return Ok(());
        };
        for (id, session_id) in waiting.drain() {
            self.answer(&id, &session_id)?;
        }
        Ok(())
    }
}
