//! The sign-in-required latch shared by every credential consumer.
//!
//! Once the gateway has rejected a credential, or the provider chain has
//! nothing left to mint from, the latch answers every background caller
//! locally and publishes the transition on a watch channel so the GUI can tell
//! the user exactly once. A refresh that merely failed to *reach* the gateway
//! does not latch — it is noted once and retried, so a laptop that wakes before
//! its network recovers without a sign-in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tokio::sync::watch;

use crate::proxy::forward::{ForwardError, ForwardResult};
use crate::{auth, config};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuthState {
    #[default]
    Ok,
    SignInRequired {
        reason: String,
    },
}

impl AuthState {
    #[must_use]
    pub const fn sign_in_required(&self) -> bool {
        matches!(self, Self::SignInRequired { .. })
    }
}

pub type CredentialStamp = auth::cache::CredentialBinding;

pub fn capture_stamp() -> ForwardResult<CredentialStamp> {
    let cfg = config::load().map_err(|e| ForwardError::Auth(e.to_string()))?;
    CredentialStamp::capture(&cfg).map_err(|e| ForwardError::Auth(e.to_string()))
}

#[derive(Debug)]
pub struct SignInLatch {
    auth_state: watch::Sender<AuthState>,
    latched_stamp: parking_lot::Mutex<Option<CredentialStamp>>,
    retry_notice: parking_lot::Mutex<Option<String>>,
}

impl Default for SignInLatch {
    fn default() -> Self {
        Self {
            auth_state: watch::Sender::new(AuthState::Ok),
            latched_stamp: parking_lot::Mutex::new(None),
            retry_notice: parking_lot::Mutex::new(None),
        }
    }
}

impl SignInLatch {
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<AuthState> {
        self.auth_state.subscribe()
    }

    #[must_use]
    pub fn engaged(&self) -> bool {
        if !self.auth_state.borrow().sign_in_required() {
            return false;
        }
        let stamped = self.latched_stamp.lock().clone();
        if stamped.is_some_and(|stamp| capture_stamp().is_ok_and(|current| stamp != current)) {
            tracing::info!("credentials changed on disk; sign-in latch released");
            self.release();
            return false;
        }
        true
    }

    pub fn engage(&self, reason: &str) {
        if self.auth_state.borrow().sign_in_required() {
            return;
        }
        tracing::warn!(reason, "token cache latched: sign-in required");
        let (stamp, reason) = match capture_stamp() {
            Ok(stamp) => (Some(stamp), reason.to_owned()),
            Err(e) => (
                None,
                format!("{reason}; credential identity unavailable: {e}"),
            ),
        };
        *self.latched_stamp.lock() = stamp;
        self.auth_state
            .send_replace(AuthState::SignInRequired { reason });
    }

    pub fn release(&self) {
        *self.latched_stamp.lock() = None;
        if self.auth_state.borrow().sign_in_required() {
            tracing::info!("token cache re-armed");
            self.auth_state.send_replace(AuthState::Ok);
        }
    }

    pub fn defer(&self, reason: &str) {
        let mut notice = self.retry_notice.lock();
        if notice.as_deref() == Some(reason) {
            return;
        }
        tracing::warn!(reason, "credential refresh deferred; will retry");
        *notice = Some(reason.to_owned());
    }

    pub fn clear_deferral(&self) {
        if self.retry_notice.lock().take().is_some() {
            tracing::info!("gateway reachable again; credential renewed without sign-in");
        }
    }
}
