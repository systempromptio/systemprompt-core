//! The heartbeat: the install's online report and the state the shell shows.
//!
//! Every 30 s while the active account is signed in (the first one interval
//! after start), the install reports itself online with the session its JWT was
//! minted for, the bridge protocol version this build speaks, the platform it
//! was built for (the compile-time `target_os` name the bridge sends:
//! `windows`, `macos`, `linux`), the machine's name and governance's usage
//! totals. A refused session is minted
//! once more and sent again. A failure is logged once until it recovers; an
//! answer that this version is unsupported or outside the instance's release
//! window is logged once while it lasts and shown in [`HeartbeatState`].
//! `BRIDGE_PROTOCOL_VERSION` is the systemprompt-models version this build
//! links, reported as the bridge protocol version.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_device::accounts::{AccountsError, AccountsSnapshot};
use systemprompt_device::client::{ClientError, CredentialError, Freshness, JwtSession};
use crate::governance::UsageTotals;
use sp_protocol::ids::AccountId;
use sp_protocol::wire::heartbeat::{
    ClientState, HEARTBEAT_INTERVAL_SECS, HeartbeatRequest, HeartbeatResponse, WindowStatus,
    saturating_total,
};
use tokio::sync::watch;
use tokio::time::{Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use crate::sync::ports::{Instance, Store};
use crate::sync::supervisor::active_signed_in;
use crate::sync::target::Target;

pub const BRIDGE_PROTOCOL_VERSION: &str = "0.87.0";

const OS: &str = std::env::consts::OS;
const UNKNOWN_HOST: &str = "unknown";
const SESSION_MARGIN: Duration = Duration::from_secs(60);

/// What the heartbeat last learned, for the shell to show.
///
/// `Paused` while nobody is signed in, `Online` after an
/// answer that admits this version, `Failing` while the heartbeat gets no
/// answer, `Unsupported` while the instance says this version is below its
/// floor or outside its release window.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HeartbeatState {
    #[default]
    Paused,
    Online,
    Failing,
    Unsupported {
        min_bridge_version: String,
        window: Option<WindowStatus>,
    },
}

/// What one heartbeat tick came to.
#[derive(Debug)]
pub enum Beat {
    Paused,
    Answered(HeartbeatResponse),
    Failed,
}

/// A change in the heartbeat worth one log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Paused,
    Failed,
    Recovered,
    Unsupported,
}

/// Folds heartbeat ticks into the [`HeartbeatState`] and the notices each
/// change is logged with.
#[derive(Debug, Default)]
pub struct Tracker {
    state: HeartbeatState,
    paused: bool,
    failing: bool,
    unsupported: Option<HeartbeatResponse>,
}

impl Tracker {
    pub const fn state(&self) -> &HeartbeatState {
        &self.state
    }

    pub fn record(&mut self, beat: Beat) -> Vec<Notice> {
        let mut notices = Vec::new();
        match beat {
            Beat::Paused => {
                if !self.paused {
                    notices.push(Notice::Paused);
                }
                self.paused = true;
                self.failing = false;
                self.unsupported = None;
                self.state = HeartbeatState::Paused;
            },
            Beat::Failed => {
                self.paused = false;
                if !self.failing {
                    notices.push(Notice::Failed);
                }
                self.failing = true;
                self.state = HeartbeatState::Failing;
            },
            Beat::Answered(answer) => {
                self.paused = false;
                if self.failing {
                    notices.push(Notice::Recovered);
                }
                self.failing = false;
                if answer.unsupported() {
                    if self.unsupported.as_ref() != Some(&answer) {
                        notices.push(Notice::Unsupported);
                    }
                    self.state = HeartbeatState::Unsupported {
                        min_bridge_version: answer.min_bridge_version.clone(),
                        window: answer.window_status,
                    };
                    self.unsupported = Some(answer);
                } else {
                    self.unsupported = None;
                    self.state = HeartbeatState::Online;
                }
            },
        }
        notices
    }
}

/// Why one heartbeat got no answer.
#[derive(Debug, thiserror::Error)]
pub enum BeatError {
    #[error("the account could not be read")]
    Accounts(#[source] AccountsError),
    #[error("no session token is available")]
    Credential(#[source] CredentialError),
    #[error("the session token names no session")]
    NoSession,
    #[error("the heartbeat call failed")]
    Client(#[source] ClientError),
}

/// What the heartbeat reports about this install besides the usage totals.
#[derive(Debug, Clone)]
pub struct Reporter {
    pub hostname: String,
    pub release_version: String,
}

impl Reporter {
    pub fn new(release_version: &str) -> Self {
        let hostname = hostname::get()
            .ok()
            .and_then(|name| name.into_string().ok())
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| UNKNOWN_HOST.to_owned());
        Self {
            hostname,
            release_version: release_version.to_owned(),
        }
    }

    pub async fn beat<S: Store>(
        &self,
        store: &S,
        account: &AccountId,
        usage: UsageTotals,
    ) -> Result<HeartbeatResponse, BeatError> {
        let instance = store.instance(account).map_err(BeatError::Accounts)?;
        let session = store
            .session(account, Freshness::ValidFor(SESSION_MARGIN))
            .await
            .map_err(BeatError::Credential)?;
        match self.send(&instance, &session, usage).await {
            Err(BeatError::Client(error)) if error.refuses_token() => {
                tracing::debug!(error = %error, "the heartbeat's session was refused; minting once more");
                let fresh = store
                    .session(account, Freshness::Fresh)
                    .await
                    .map_err(BeatError::Credential)?;
                self.send(&instance, &fresh, usage).await
            },
            other => other,
        }
    }

    async fn send<I: Instance>(
        &self,
        instance: &I,
        session: &JwtSession,
        usage: UsageTotals,
    ) -> Result<HeartbeatResponse, BeatError> {
        let session_id = session.session_id.clone().ok_or(BeatError::NoSession)?;
        let request = HeartbeatRequest {
            session_id,
            bridge_version: BRIDGE_PROTOCOL_VERSION.to_owned(),
            os: OS.to_owned(),
            hostname: self.hostname.clone(),
            last_activity_at: usage.last_activity_at,
            forwarded_total: saturating_total(usage.forwarded_total),
            tokens_in_total: saturating_total(usage.tokens_in_total),
            tokens_out_total: saturating_total(usage.tokens_out_total),
            release_version: self.release_version.clone(),
            state: ClientState::Active,
        };
        instance
            .heartbeat(&session.jwt, &request)
            .await
            .map_err(BeatError::Client)
    }
}

pub(crate) struct Heartbeat<S: Store, T: Target> {
    pub(crate) store: S,
    pub(crate) target: T,
    pub(crate) reporter: Reporter,
    pub(crate) snapshots: watch::Receiver<AccountsSnapshot>,
    pub(crate) state: Arc<watch::Sender<HeartbeatState>>,
}

impl<S: Store, T: Target> Heartbeat<S, T> {
    pub(crate) async fn run(self, cancel: CancellationToken) {
        let period = Duration::from_secs(HEARTBEAT_INTERVAL_SECS);
        let mut ticks = tokio::time::interval_at(Instant::now() + period, period);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut tracker = Tracker::default();
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = ticks.tick() => {},
            }
            let active = active_signed_in(&self.snapshots.borrow()).map(|view| view.id.clone());
            let beat = match active {
                None => Beat::Paused,
                Some(account) => {
                    let answered = tokio::select! {
                        () = cancel.cancelled() => break,
                        answered = self.reporter.beat(&self.store, &account, self.target.usage()) => answered,
                    };
                    match answered {
                        Ok(answer) => Beat::Answered(answer),
                        Err(BeatError::Credential(CredentialError::SignedOut)) => continue,
                        Err(error) => {
                            if tracker.state() != &HeartbeatState::Failing {
                                tracing::warn!(account = %account, error = %error, "the heartbeat failed");
                            }
                            Beat::Failed
                        },
                    }
                },
            };
            for notice in tracker.record(beat) {
                self.log(notice, tracker.state());
            }
            self.state.send_replace(tracker.state().clone());
        }
        tracing::debug!("heartbeat stopped");
    }

    fn log(&self, notice: Notice, state: &HeartbeatState) {
        match notice {
            Notice::Paused => tracing::info!("the heartbeat pauses until an account is signed in"),
            Notice::Failed => {},
            Notice::Recovered => tracing::info!("the heartbeat recovered"),
            Notice::Unsupported => tracing::warn!(
                bridge_version = BRIDGE_PROTOCOL_VERSION,
                release = %self.reporter.release_version,
                state = ?state,
                "the instance reports this version as outside the versions it supports"
            ),
        }
    }
}
