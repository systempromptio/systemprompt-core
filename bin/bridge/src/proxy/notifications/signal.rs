//! Typed notification signals the comms stream hands to the inbox task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use serde_json::value::RawValue;
use systemprompt_models::notifications::{
    NOTIFICATION_CREATED, NOTIFICATION_ESCALATED, NOTIFICATION_READ, NotificationCreatedPayload,
    NotificationEscalatedPayload, NotificationReadPayload,
};
use tokio::sync::mpsc;

const CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub enum NotificationSignal {
    Created(Box<NotificationCreatedPayload>),
    Read(NotificationReadPayload),
    Escalated(NotificationEscalatedPayload),
    StreamOpened,
}

#[derive(Debug, Clone)]
pub struct NotificationSignals {
    tx: mpsc::Sender<NotificationSignal>,
}

pub type NotificationSignalReceiver = mpsc::Receiver<NotificationSignal>;

impl NotificationSignals {
    #[must_use]
    pub fn channel() -> (Self, NotificationSignalReceiver) {
        let (tx, rx) = mpsc::channel(CAPACITY);
        (Self { tx }, rx)
    }

    // Why: a signal dropped on a full channel is recovered by the catch-up the
    // next stream reopen or GUI open runs against the stored cursor.
    pub fn send(&self, signal: NotificationSignal) {
        if let Err(error) = self.tx.try_send(signal) {
            tracing::debug!(%error, "notification signal not delivered; catch-up covers it");
        }
    }
}

#[must_use]
pub fn signal_for(name: &str, value: &RawValue) -> Option<NotificationSignal> {
    match name {
        NOTIFICATION_CREATED => decode::<NotificationCreatedPayload>(name, value)
            .map(|p| NotificationSignal::Created(Box::new(p))),
        NOTIFICATION_READ => decode(name, value).map(NotificationSignal::Read),
        NOTIFICATION_ESCALATED => decode(name, value).map(NotificationSignal::Escalated),
        _ => None,
    }
}

fn decode<T: DeserializeOwned>(name: &str, value: &RawValue) -> Option<T> {
    match serde_json::from_str(value.get()) {
        Ok(decoded) => Some(decoded),
        Err(error) => {
            tracing::warn!(%error, event = name, "notification frame did not decode; it is skipped");
            None
        },
    }
}
