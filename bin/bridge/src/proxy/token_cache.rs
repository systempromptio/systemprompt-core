//! Cached gateway JWT with background refresh ahead of expiry.
//!
//! The cache also owns the *sign-in-required* latch. Once the gateway has
//! rejected a freshly minted token, or the provider chain has nothing left to
//! mint from, every background caller (refresh tick, heartbeat, comms stream,
//! forwarded requests) is answered from the latch without touching the
//! network, and the transition is published on a watch channel so the GUI can
//! tell the user exactly once. Only an explicit sign-in re-arms minting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, watch};

use systemprompt_identifiers::SessionId;

use crate::gateway::types::HelperOutput;
use crate::proxy::forward::{ForwardError, ForwardResult};
use crate::{auth, config};

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);
const FRESH_REJECTION_WINDOW: Duration = Duration::from_secs(120);

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

type CredentialStamp = auth::cache::CredentialBinding;

fn capture_stamp() -> ForwardResult<CredentialStamp> {
    let cfg = config::load().map_err(|e| ForwardError::Auth(e.to_string()))?;
    CredentialStamp::capture(&cfg).map_err(|e| ForwardError::Auth(e.to_string()))
}

pub type RefreshFn = Arc<
    dyn Fn(u64) -> Pin<Box<dyn Future<Output = ForwardResult<HelperOutput>> + Send>> + Send + Sync,
>;

struct CachedEntry {
    token: HelperOutput,
    minted_at: Instant,
    stamp: CredentialStamp,
}

#[expect(
    missing_debug_implementations,
    reason = "holds a `dyn Fn -> Pin<Box<Future>>` refresh callback; cannot derive Debug"
)]
pub struct TokenCache {
    cached: Mutex<Option<CachedEntry>>,
    refresh_lock: Mutex<()>,
    refresh: RefreshFn,
    auth_state: watch::Sender<AuthState>,
    latched_stamp: parking_lot::Mutex<Option<CredentialStamp>>,
}

impl TokenCache {
    #[must_use]
    pub fn new(refresh: RefreshFn) -> Self {
        Self {
            cached: Mutex::new(None),
            refresh_lock: Mutex::new(()),
            refresh,
            auth_state: watch::Sender::new(AuthState::Ok),
            latched_stamp: parking_lot::Mutex::new(None),
        }
    }

    #[must_use]
    pub fn auth_state(&self) -> watch::Receiver<AuthState> {
        self.auth_state.subscribe()
    }

    #[must_use]
    pub fn sign_in_required(&self) -> bool {
        if !self.auth_state.borrow().sign_in_required() {
            return false;
        }
        let stamped = self.latched_stamp.lock().clone();
        if stamped.is_some_and(|stamp| capture_stamp().is_ok_and(|current| stamp != current)) {
            tracing::info!("credentials changed on disk; sign-in latch released");
            self.unlatch();
            return false;
        }
        true
    }

    fn latch(&self, reason: &str) {
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

    fn unlatch(&self) {
        *self.latched_stamp.lock() = None;
        if self.auth_state.borrow().sign_in_required() {
            tracing::info!("token cache re-armed");
            self.auth_state.send_replace(AuthState::Ok);
        }
    }

    pub async fn refresh_if_cached(&self, refresh_threshold_secs: u64) -> ForwardResult<()> {
        if self.cached.lock().await.is_none() {
            return Ok(());
        }
        self.current(refresh_threshold_secs).await.map(|_| ())
    }

    #[must_use]
    pub fn default_for_runtime(session_id: SessionId, http: reqwest::Client) -> Self {
        Self::new(Arc::new(move |threshold| {
            let session_id = session_id.clone();
            let http = http.clone();
            Box::pin(async move {
                let cfg = config::load().map_err(|e| ForwardError::Auth(e.to_string()))?;
                auth::read_or_refresh(&cfg, threshold, &session_id, &http)
                    .await
                    .map_err(|e| ForwardError::Auth(e.to_string()))
            })
        }))
    }

    #[expect(
        clippy::significant_drop_tightening,
        reason = "refresh_guard intentionally held to serialise concurrent refreshes"
    )]
    pub async fn current(&self, refresh_threshold_secs: u64) -> ForwardResult<HelperOutput> {
        if let Some(token) = self.peek_fresh(refresh_threshold_secs).await? {
            return Ok(token);
        }
        if self.sign_in_required() {
            return Err(sign_in_required_error());
        }

        let _refresh_guard = self.refresh_lock.lock().await;

        if let Some(token) = self.peek_fresh(refresh_threshold_secs).await? {
            return Ok(token);
        }
        if self.sign_in_required() {
            return Err(sign_in_required_error());
        }

        let stamp = capture_stamp()?;
        let refresh = Arc::clone(&self.refresh);
        let token = tokio::time::timeout(REFRESH_TIMEOUT, refresh(refresh_threshold_secs))
            .await
            .map_err(|_elapsed| ForwardError::AuthTimeout)?
            .inspect_err(|e| {
                self.latch(&e.to_string());
            })?;
        if capture_stamp()? != stamp {
            return Err(ForwardError::Auth(
                "credentials changed during token refresh".into(),
            ));
        }

        tracing::info!("token cache refresh");
        self.unlatch();

        let mut guard = self.cached.lock().await;
        *guard = Some(CachedEntry {
            token: token.clone(),
            minted_at: Instant::now(),
            stamp,
        });
        Ok(token)
    }

    pub async fn invalidate(&self) {
        let mut guard = self.cached.lock().await;
        if guard.is_some() {
            tracing::info!("token cache invalidated (upstream rejected JWT)");
            *guard = None;
        }
    }

    pub async fn reject_upstream(&self, endpoint: &str) {
        let mut guard = self.cached.lock().await;
        let Some(entry) = guard.take() else {
            return;
        };
        if entry.minted_at.elapsed() <= FRESH_REJECTION_WINDOW {
            drop(guard);
            self.latch(&format!(
                "{endpoint} rejected a credential issued {}s ago",
                entry.minted_at.elapsed().as_secs()
            ));
        } else {
            tracing::info!(endpoint, "token cache invalidated (upstream rejected JWT)");
        }
    }

    pub async fn reset(&self) {
        self.invalidate().await;
        self.unlatch();
    }

    #[expect(
        clippy::significant_drop_tightening,
        reason = "guard scope is the whole function; entry borrows from it"
    )]
    async fn peek_fresh(&self, refresh_threshold_secs: u64) -> ForwardResult<Option<HelperOutput>> {
        let mut guard = self.cached.lock().await;
        let Some(entry) = guard.as_mut() else {
            return Ok(None);
        };
        let age_secs = entry.minted_at.elapsed().as_secs();
        if age_secs.saturating_add(refresh_threshold_secs) >= entry.token.ttl {
            return Ok(None);
        }
        {
            let current = capture_stamp()?;
            if current != entry.stamp {
                tracing::info!("credentials changed on disk; discarding cached token");
                *guard = None;
                return Ok(None);
            }
        }
        tracing::debug!(cached_age_secs = age_secs, "token cache hit");
        Ok(Some(entry.token.clone()))
    }
}

fn sign_in_required_error() -> ForwardError {
    ForwardError::Auth(format!(
        "no JWT available — sign in via {} GUI",
        crate::brand::brand().app_name
    ))
}
