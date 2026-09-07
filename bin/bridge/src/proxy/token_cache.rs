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
const STAMP_CHECK_INTERVAL: Duration = Duration::from_secs(5);
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

#[derive(Clone, PartialEq, Eq)]
struct CredentialStamp {
    pat_mtime: Option<std::time::SystemTime>,
    config_mtime: Option<std::time::SystemTime>,
}

impl CredentialStamp {
    fn capture() -> Self {
        let pat = auth::setup::resolve_paths().ok().map(|p| p.pat_file);
        Self {
            pat_mtime: mtime(pat),
            config_mtime: mtime(config::config_path()),
        }
    }
}

fn mtime(path: Option<std::path::PathBuf>) -> Option<std::time::SystemTime> {
    path.and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
}

pub type RefreshFn =
    Arc<dyn Fn(u64) -> Pin<Box<dyn Future<Output = Option<HelperOutput>> + Send>> + Send + Sync>;

struct CachedEntry {
    token: HelperOutput,
    minted_at: Instant,
    stamp: CredentialStamp,
    stamp_checked_at: Instant,
}

#[expect(
    missing_debug_implementations,
    reason = "holds a `dyn Fn -> Pin<Box<Future>>` refresh callback; cannot derive Debug"
)]
pub struct TokenCache {
    cached: Mutex<Option<CachedEntry>>,
    refresh_lock: Mutex<()>,
    refresh: RefreshFn,
    stamp_check_interval: Duration,
    auth_state: watch::Sender<AuthState>,
    latched_stamp: std::sync::Mutex<Option<CredentialStamp>>,
}

impl TokenCache {
    #[must_use]
    pub fn new(refresh: RefreshFn) -> Self {
        Self {
            cached: Mutex::new(None),
            refresh_lock: Mutex::new(()),
            refresh,
            stamp_check_interval: STAMP_CHECK_INTERVAL,
            auth_state: watch::Sender::new(AuthState::Ok),
            latched_stamp: std::sync::Mutex::new(None),
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
        let stamped = self.latched_stamp.lock().ok().and_then(|g| g.clone());
        if stamped.is_some_and(|stamp| stamp != CredentialStamp::capture()) {
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
        if let Ok(mut guard) = self.latched_stamp.lock() {
            *guard = Some(CredentialStamp::capture());
        }
        self.auth_state.send_replace(AuthState::SignInRequired {
            reason: reason.to_owned(),
        });
    }

    fn unlatch(&self) {
        if let Ok(mut guard) = self.latched_stamp.lock() {
            *guard = None;
        }
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
    pub const fn with_stamp_check_interval(mut self, interval: Duration) -> Self {
        self.stamp_check_interval = interval;
        self
    }

    #[must_use]
    pub fn default_for_runtime(session_id: SessionId, http: reqwest::Client) -> Self {
        Self::new(Arc::new(move |threshold| {
            let session_id = session_id.clone();
            let http = http.clone();
            Box::pin(async move {
                let cfg = config::load();
                auth::read_or_refresh(&cfg, threshold, &session_id, &http).await
            })
        }))
    }

    #[expect(
        clippy::significant_drop_tightening,
        reason = "refresh_guard intentionally held to serialise concurrent refreshes"
    )]
    pub async fn current(&self, refresh_threshold_secs: u64) -> ForwardResult<HelperOutput> {
        if let Some(token) = self.peek_fresh(refresh_threshold_secs).await {
            return Ok(token);
        }
        if self.sign_in_required() {
            return Err(sign_in_required_error());
        }

        let _refresh_guard = self.refresh_lock.lock().await;

        if let Some(token) = self.peek_fresh(refresh_threshold_secs).await {
            return Ok(token);
        }
        if self.sign_in_required() {
            return Err(sign_in_required_error());
        }

        let refresh = Arc::clone(&self.refresh);
        let token = tokio::time::timeout(REFRESH_TIMEOUT, refresh(refresh_threshold_secs))
            .await
            .map_err(|_elapsed| ForwardError::AuthTimeout)?
            .ok_or_else(|| {
                self.latch("no credential source could mint a token");
                sign_in_required_error()
            })?;

        tracing::info!("token cache refresh");
        self.unlatch();

        let mut guard = self.cached.lock().await;
        *guard = Some(CachedEntry {
            token: token.clone(),
            minted_at: Instant::now(),
            stamp: CredentialStamp::capture(),
            stamp_checked_at: Instant::now(),
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
    async fn peek_fresh(&self, refresh_threshold_secs: u64) -> Option<HelperOutput> {
        let mut guard = self.cached.lock().await;
        let entry = guard.as_mut()?;
        let age_secs = entry.minted_at.elapsed().as_secs();
        if age_secs.saturating_add(refresh_threshold_secs) >= entry.token.ttl {
            return None;
        }
        if entry.stamp_checked_at.elapsed() >= self.stamp_check_interval {
            let current = CredentialStamp::capture();
            if current != entry.stamp {
                tracing::info!("credentials changed on disk; discarding cached token");
                *guard = None;
                return None;
            }
            entry.stamp_checked_at = Instant::now();
        }
        tracing::debug!(cached_age_secs = age_secs, "token cache hit");
        Some(entry.token.clone())
    }
}

fn sign_in_required_error() -> ForwardError {
    ForwardError::Auth(format!(
        "no JWT available — sign in via {} GUI",
        crate::brand::brand().app_name
    ))
}
