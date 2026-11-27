//! Why a run stops before or between its prompts, and the wait for the active
//! instance to be applied before the first one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error as StdError;
use std::time::Duration;

use systemprompt_device::view::{SyncErrorKind, SyncStatus};
use systemprompt_engine::run::EngineError;
use systemprompt_engine::store::SessionId;
use systemprompt_traits::CancellationToken;

use crate::command::compose::Composition;
use crate::command::exit::ExitStatus;
use crate::command::pump::PumpError;
use crate::command::run::RunReport;
use crate::sync::SyncEngine;

pub(crate) struct Stop {
    pub(crate) status: ExitStatus,
    pub(crate) message: Option<String>,
}

impl Stop {
    pub(crate) fn new(status: ExitStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: Some(message.into()),
        }
    }

    pub(crate) fn of_engine(error: &EngineError) -> Self {
        Self::new(ExitStatus::of_engine(error.kind()), error.describe())
    }

    pub(crate) fn of_pump(error: PumpError) -> Self {
        match error {
            PumpError::Engine(error) => Self::of_engine(&error),
            other => Self::new(ExitStatus::Internal, describe(&other)),
        }
    }

    pub(crate) fn report(self, session_id: Option<SessionId>) -> RunReport {
        RunReport {
            status: self.status,
            session_id,
            message: self.message,
        }
    }
}

pub(crate) fn describe(error: &dyn StdError) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

pub(crate) async fn ready(
    composition: &Composition,
    limit: Duration,
    cancel: &CancellationToken,
) -> Result<(), Stop> {
    if composition.accounts.snapshot().needs_sign_in() {
        return Err(Stop::new(
            ExitStatus::NotSignedIn,
            "no account is signed in; run login first",
        ));
    }
    applied(&composition.sync, limit, cancel).await
}

const fn sync_failure(kind: SyncErrorKind) -> &'static str {
    match kind {
        SyncErrorKind::Unreachable => "the instance could not be reached",
        SyncErrorKind::RateLimited => "the instance is rate limiting this device",
        SyncErrorKind::Refused => "the instance refused the stored credential",
        SyncErrorKind::KeyChanged => "the instance signs with a manifest key that is not trusted",
        SyncErrorKind::Invalid => "the instance served a manifest that was not accepted",
        SyncErrorKind::Insecure => "the stored credential could not be opened",
        SyncErrorKind::Service => "the instance's content could not be applied",
    }
}

async fn applied(
    sync: &SyncEngine,
    limit: Duration,
    cancel: &CancellationToken,
) -> Result<(), Stop> {
    let mut status = sync.subscribe();
    let settled = async {
        loop {
            let seen = match &*status.borrow_and_update() {
                SyncStatus::Synced { .. } => Some(Ok(())),
                SyncStatus::Error { kind, .. } => Some(Err(Stop::new(
                    ExitStatus::of_sync(*kind),
                    sync_failure(*kind),
                ))),
                SyncStatus::Idle | SyncStatus::Syncing { .. } => None,
            };
            if let Some(outcome) = seen {
                return outcome;
            }
            if status.changed().await.is_err() {
                return Err(Stop::new(ExitStatus::Internal, "the sync stopped"));
            }
        }
    };
    tokio::select! {
        outcome = settled => outcome,
        () = tokio::time::sleep(limit) => Err(Stop::new(
            ExitStatus::Unavailable,
            format!("the instance was not applied within {} s", limit.as_secs()),
        )),
        () = cancel.cancelled() => Err(Stop {
            status: ExitStatus::Aborted,
            message: None,
        }),
    }
}
