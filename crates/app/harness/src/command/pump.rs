//! The loop that turns one run's two feeds into its event stream and answers
//! what the engine asks.
//!
//! The store's change feed and the engine's event feed are read together until
//! the session is idle again; the store feed is then drained, so every write
//! of the turn is on the stream before its outcome is read. A feed that lagged
//! is reported, and after a lag on the engine's feed the requests still
//! waiting are answered and the run status is read directly, so a dropped
//! event cannot leave the run waiting. Under a control feed a permission
//! request is written and held until the feed replies to it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, Write};
use std::time::Duration;

use systemprompt_engine::run::{
    Engine, EngineError, EngineEvent, PermissionAsk, PromptInput, QuestionAsk, RunStatus,
    SessionFailure,
};
use systemprompt_engine::store::{Change, Message, MessageWithParts, SessionId};
use systemprompt_identifiers::engine::PermissionId;
use systemprompt_traits::CancellationToken;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::broadcast::{self};
use tokio::time::Instant;

use crate::command::control::{self, ControlError, ControlFeed};
use crate::command::event::{EventWriter, Feed, RunEvent};
use crate::command::exit::ExitStatus;
use crate::command::permission::{Decider, WaitingAsks};

const TITLE_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub(crate) enum PumpError {
    #[error("the event stream could not be written")]
    Output(#[source] io::Error),
    #[error("a feed of the engine closed while the session was running")]
    Closed,
    #[error("a control could not be carried out")]
    Engine(#[source] EngineError),
    #[error("the control stream stopped the run")]
    Control(#[source] ControlError),
}

pub(crate) struct Limits<'a> {
    pub(crate) cancel: &'a CancellationToken,
    pub(crate) deadline: Option<Instant>,
}

pub(crate) struct Feeds {
    pub(crate) changes: broadcast::Receiver<Change>,
    pub(crate) events: broadcast::Receiver<EngineEvent>,
    pub(crate) controls: Option<ControlFeed>,
}

pub(crate) struct Pump<'a, W: Write + Send> {
    pub(crate) engine: &'a Engine,
    pub(crate) root: SessionId,
    pub(crate) decider: &'a mut Decider,
    pub(crate) waiting: Option<WaitingAsks>,
    pub(crate) out: &'a mut EventWriter<W>,
    pub(crate) feeds: Feeds,
    pub(crate) template: PromptInput,
    pub(crate) failure: Option<SessionFailure>,
    pub(crate) interrupted: Option<ExitStatus>,
    pub(crate) titled: bool,
}

pub(crate) async fn expired(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

impl<W: Write + Send> Pump<'_, W> {
    pub(crate) fn emit(&mut self, event: &RunEvent) -> Result<(), PumpError> {
        self.out.emit(event).map_err(PumpError::Output)
    }

    pub(crate) async fn until_idle(&mut self, limits: &Limits<'_>) -> Result<(), PumpError> {
        loop {
            tokio::select! {
                biased;
                change = self.feeds.changes.recv() => self.on_change(change)?,
                event = self.feeds.events.recv() => {
                    if self.on_event(event)? {
                        break;
                    }
                },
                command = control::next(&mut self.feeds.controls) => self.carry_out(command?).await?,
                () = limits.cancel.cancelled(), if self.interrupted.is_none() => {
                    self.interrupt(ExitStatus::Aborted);
                },
                () = expired(limits.deadline), if self.interrupted.is_none() => {
                    self.interrupt(ExitStatus::TimedOut);
                },
            }
        }
        self.drain()
    }

    /// Waits for the title of the first prompt, for at most `TITLE_GRACE`.
    ///
    /// An aborted run waits too: the abort stops the step, not the title
    /// call beside it, and ending first would shut the engine down under
    /// that call. A run that outran its own timeout does not wait.
    pub(crate) async fn settle_title(&mut self) -> Result<(), PumpError> {
        let deadline = Instant::now() + TITLE_GRACE;
        while !self.titled && self.interrupted != Some(ExitStatus::TimedOut) {
            tokio::select! {
                change = self.feeds.changes.recv() => self.on_change(change)?,
                () = tokio::time::sleep_until(deadline) => break,
            }
        }
        Ok(())
    }

    pub(crate) fn outcome(
        &self,
        messages: Result<Vec<MessageWithParts>, EngineError>,
    ) -> (ExitStatus, Option<String>) {
        if let Some(status) = self.interrupted {
            return (status, None);
        }
        let messages = match messages {
            Ok(messages) => messages,
            Err(error) => return (ExitStatus::of_engine(error.kind()), Some(error.describe())),
        };
        let last_error = messages
            .iter()
            .rev()
            .find_map(|entry| match &entry.message {
                Message::Assistant(message) => Some(message.error.clone()),
                Message::User(_) => None,
            })
            .flatten();
        if let Some(error) = last_error {
            return (ExitStatus::of_message(error.kind), Some(error.message));
        }
        self.failure
            .as_ref()
            .map_or((ExitStatus::Completed, None), |failure| {
                (
                    ExitStatus::of_engine(failure.kind),
                    Some(failure.message.clone()),
                )
            })
    }

    pub(crate) fn interrupt(&mut self, status: ExitStatus) {
        self.interrupted = Some(status);
        let running = self.engine.abort(&self.root);
        tracing::info!(session = %self.root, running, outcome = ?status, "the run was interrupted");
    }

    pub(crate) fn drain(&mut self) -> Result<(), PumpError> {
        loop {
            match self.feeds.changes.try_recv() {
                Ok(change) => self.on_change(Ok(change))?,
                Err(TryRecvError::Lagged(missed)) => {
                    self.on_change(Err(RecvError::Lagged(missed)))?;
                },
                Err(TryRecvError::Empty | TryRecvError::Closed) => return Ok(()),
            }
        }
    }

    pub(crate) fn on_change(&mut self, change: Result<Change, RecvError>) -> Result<(), PumpError> {
        match change {
            Ok(change) => {
                if let Change::SessionCreated(session) | Change::SessionUpdated(session) = &change {
                    self.titled |= session.id == self.root && !session.title.is_empty();
                }
                RunEvent::of_change(change).map_or(Ok(()), |event| self.emit(&event))
            },
            Err(RecvError::Lagged(missed)) => self.emit(&RunEvent::Lagged {
                feed: Feed::Store,
                missed,
            }),
            Err(RecvError::Closed) => Err(PumpError::Closed),
        }
    }

    pub(crate) fn on_event(
        &mut self,
        event: Result<EngineEvent, RecvError>,
    ) -> Result<bool, PumpError> {
        match event {
            Ok(EngineEvent::PermissionResolved { id, .. }) => {
                if let Some(waiting) = &mut self.waiting {
                    waiting.release(&id);
                }
                Ok(false)
            },
            Ok(EngineEvent::PermissionAsked(ask)) => self.permission(&ask).map(|()| false),
            Ok(EngineEvent::QuestionAsked(ask)) => self.question(&ask).map(|()| false),
            Ok(event) => self.on_engine(event),
            Err(RecvError::Lagged(missed)) => {
                self.emit(&RunEvent::Lagged {
                    feed: Feed::Engine,
                    missed,
                })?;
                for ask in self.engine.pending_permissions() {
                    self.permission(&ask)?;
                }
                for ask in self.engine.pending_questions() {
                    self.question(&ask)?;
                }
                Ok(self.engine.status(&self.root) == RunStatus::Idle)
            },
            Err(RecvError::Closed) => Err(PumpError::Closed),
        }
    }

    fn on_engine(&mut self, event: EngineEvent) -> Result<bool, PumpError> {
        let idle = matches!(
            &event,
            EngineEvent::Status { session, status: RunStatus::Idle } if session == &self.root
        );
        if let EngineEvent::SessionError { session, error } = &event
            && session == &self.root
        {
            self.failure = Some(error.clone());
        }
        if let Some(event) = RunEvent::of_engine(event) {
            self.emit(&event)?;
        }
        Ok(idle)
    }

    fn permission(&mut self, ask: &PermissionAsk) -> Result<(), PumpError> {
        let held = self
            .waiting
            .as_mut()
            .map(|waiting| waiting.hold(&ask.id, &ask.session));
        if held == Some(false) {
            return Ok(());
        }
        self.emit(&RunEvent::PermissionAsked {
            ask: Box::new(ask.clone()),
        })?;
        if held.is_some() {
            return Ok(());
        }
        self.answer(&ask.id, &ask.session)
    }

    pub(crate) fn answer(
        &mut self,
        id: &PermissionId,
        session: &SessionId,
    ) -> Result<(), PumpError> {
        let (reply, source) = self.decider.permission();
        if !self.engine.reply_permission(id, reply.clone()) {
            return Ok(());
        }
        self.emit(&RunEvent::PermissionReplied {
            id: id.clone(),
            session_id: session.clone(),
            reply,
            source,
        })
    }

    fn question(&mut self, ask: &QuestionAsk) -> Result<(), PumpError> {
        self.emit(&RunEvent::QuestionAsked { ask: ask.clone() })?;
        let (answers, source) = self.decider.question();
        if !self.engine.answer_question(&ask.id, answers.clone()) {
            return Ok(());
        }
        self.emit(&RunEvent::QuestionAnswered {
            id: ask.id.clone(),
            session_id: ask.session.clone(),
            answers,
            source,
        })
    }
}
