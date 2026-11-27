//! What a caller may tell a run while it is in progress: a further prompt, an
//! abort, a revert, or that nothing more is coming.
//!
//! A run given a control feed stays open after its prompts until the feed
//! closes or sends `close`. A prompt sent while the session is busy is
//! steered into the running turn or queued behind it; one sent while it is
//! idle starts a turn. A revert is honoured only while the session is idle.
//! The caller decides when to send each from the event stream it reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;

use serde::Deserialize;
use systemprompt_engine::run::{Delivery, PromptInput};
use systemprompt_engine::store::MessageId;
use tokio::sync::mpsc;

use crate::command::exit::ExitStatus;
use crate::command::pump::{Limits, Pump, PumpError, expired};

/// The feed a caller sends [`Control`]s on.
pub type ControlFeed = mpsc::UnboundedReceiver<Control>;

/// How a prompt sent to a busy session is admitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlDelivery {
    #[default]
    Steer,
    Queue,
}

/// One instruction to a run in progress; on a line it is a JSON object with a
/// `type`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Control {
    Prompt {
        text: String,
        #[serde(default)]
        delivery: ControlDelivery,
    },
    Abort,
    Revert {
        message_id: MessageId,
    },
    Close,
}

impl Control {
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

pub(crate) async fn next(controls: &mut Option<ControlFeed>) -> Control {
    match controls {
        Some(feed) => feed.recv().await.unwrap_or(Control::Close),
        None => std::future::pending().await,
    }
}

impl<W: Write + Send> Pump<'_, W> {
    pub(crate) async fn during_turn(&mut self, control: Control) -> Result<(), PumpError> {
        match control {
            Control::Prompt { text, delivery } => {
                let input = PromptInput {
                    text,
                    delivery: match delivery {
                        ControlDelivery::Steer => Delivery::Steer,
                        ControlDelivery::Queue => Delivery::Queue,
                    },
                    ..self.template.clone()
                };
                let admitted = self.engine.prompt(input).await;
                admitted.map(|_| ()).map_err(PumpError::Engine)
            },
            Control::Abort => {
                self.interrupt(ExitStatus::Aborted);
                Ok(())
            },
            Control::Revert { message_id } => {
                let reverted = self.engine.revert(&self.root, &message_id).await;
                reverted.map(|_| ()).map_err(PumpError::Engine)
            },
            Control::Close => {
                self.feeds.controls = None;
                Ok(())
            },
        }
    }

    pub(crate) async fn while_idle(
        &mut self,
        limits: &Limits<'_>,
    ) -> Result<Option<Control>, PumpError> {
        if self.feeds.controls.is_none() {
            return Ok(None);
        }
        loop {
            tokio::select! {
                change = self.feeds.changes.recv() => self.on_change(change)?,
                control = next(&mut self.feeds.controls) => {
                    if control == Control::Close {
                        self.feeds.controls = None;
                        return Ok(None);
                    }
                    return Ok(Some(control));
                },
                () = limits.cancel.cancelled() => {
                    self.interrupted = Some(ExitStatus::Aborted);
                    return Ok(None);
                },
                () = expired(limits.deadline) => {
                    self.interrupted = Some(ExitStatus::TimedOut);
                    return Ok(None);
                },
            }
        }
    }
}
