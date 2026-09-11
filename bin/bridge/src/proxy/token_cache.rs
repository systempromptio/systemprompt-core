//! Cached gateway JWT with background refresh ahead of expiry.
//!
//! Minting is guarded by the shared [`SignInLatch`]: a terminal failure latches
//! and every later caller is answered locally, while a failure that merely
//! could not reach the gateway is deferred and retried on the next tick.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use systemprompt_identifiers::SessionId;

use crate::gateway::types::HelperOutput;
use crate::proxy::forward::{ForwardError, ForwardResult};
use crate::proxy::sign_in_latch::{CredentialStamp, SignInLatch, capture_stamp};
use crate::{auth, config};

pub use crate::proxy::sign_in_latch::AuthState;

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);
const FRESH_REJECTION_WINDOW: Duration = Duration::from_secs(120);
// Why: the credential stamp reads the config file, hashes the PAT and opens
// the keystore. Doing that on every forwarded request put a disk read and a
// keychain call on the hot path; once per interval catches a rotated
// credential within seconds without paying for it per request.
const STAMP_CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub type RefreshFn = Arc<
    dyn Fn(u64) -> Pin<Box<dyn Future<Output = ForwardResult<HelperOutput>> + Send>> + Send + Sync,
>;

struct CachedEntry {
    token: HelperOutput,
    minted_at: Instant,
    stamp: CredentialStamp,
    // Why: tokio's clock, not std's, so a paused test clock drives the
    // interval; `minted_at` stays on std time because the rejection window
    // measures real elapsed time against the gateway.
    stamp_checked_at: tokio::time::Instant,
}

#[expect(
    missing_debug_implementations,
    reason = "holds a `dyn Fn -> Pin<Box<Future>>` refresh callback; cannot derive Debug"
)]
pub struct TokenCache {
    cached: Mutex<Option<CachedEntry>>,
    refresh_lock: Mutex<()>,
    refresh: RefreshFn,
    latch: SignInLatch,
    // Why: a runtime-config swap (gateway change, login, logout) bumps this so
    // long-lived streams opened against the previous gateway can end instead
    // of living on until that gateway drops them.
    generation: tokio::sync::watch::Sender<u64>,
}

impl TokenCache {
    #[must_use]
    pub fn new(refresh: RefreshFn) -> Self {
        Self {
            cached: Mutex::new(None),
            refresh_lock: Mutex::new(()),
            refresh,
            latch: SignInLatch::default(),
            generation: tokio::sync::watch::Sender::new(0),
        }
    }

    /// A receiver that fires on every [`Self::reset`]; hold one across a
    /// long-lived upstream connection and drop the connection when it fires.
    #[must_use]
    pub fn generation(&self) -> tokio::sync::watch::Receiver<u64> {
        self.generation.subscribe()
    }

    #[must_use]
    pub fn auth_state(&self) -> tokio::sync::watch::Receiver<AuthState> {
        self.latch.subscribe()
    }

    #[must_use]
    pub fn sign_in_required(&self) -> bool {
        self.latch.engaged()
    }

    /// Releases the sign-in latch on live proof that the credential mints:
    /// the GUI probe just obtained a token from the gateway, so whatever
    /// rejected the previous one was not the credential.
    pub fn credential_proven(&self) {
        self.latch.release();
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
                    .map_err(|e| chain_error(&e))
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
            .inspect_err(|e| match e {
                ForwardError::AuthRetryable(reason) => self.latch.defer(reason),
                terminal => self.latch.engage(&terminal.to_string()),
            })?;
        if capture_stamp()? != stamp {
            return Err(ForwardError::Auth(
                "credentials changed during token refresh".into(),
            ));
        }

        tracing::info!("token cache refresh");
        self.latch.clear_deferral();
        self.latch.release();

        let mut guard = self.cached.lock().await;
        *guard = Some(CachedEntry {
            token: token.clone(),
            minted_at: Instant::now(),
            stamp,
            stamp_checked_at: tokio::time::Instant::now(),
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
            self.latch.engage(&format!(
                "{endpoint} rejected a credential issued {}s ago",
                entry.minted_at.elapsed().as_secs()
            ));
        } else {
            tracing::info!(endpoint, "token cache invalidated (upstream rejected JWT)");
        }
    }

    pub async fn reset(&self) {
        self.invalidate().await;
        self.latch.release();
        self.generation.send_modify(|g| *g += 1);
    }

    #[expect(
        clippy::significant_drop_tightening,
        reason = "the guard is released before the blocking stamp capture and re-taken after; \
                  the scopes are the point"
    )]
    async fn peek_fresh(&self, refresh_threshold_secs: u64) -> ForwardResult<Option<HelperOutput>> {
        let (token, age_secs) = {
            let mut guard = self.cached.lock().await;
            let Some(entry) = guard.as_mut() else {
                return Ok(None);
            };
            let age_secs = entry.minted_at.elapsed().as_secs();
            if age_secs.saturating_add(refresh_threshold_secs) >= entry.token.ttl {
                return Ok(None);
            }
            if entry.stamp_checked_at.elapsed() < STAMP_CHECK_INTERVAL {
                tracing::debug!(cached_age_secs = age_secs, "token cache hit");
                return Ok(Some(entry.token.clone()));
            }
            (entry.token.clone(), age_secs)
        };
        let current = tokio::task::spawn_blocking(capture_stamp)
            .await
            .map_err(|e| ForwardError::Auth(format!("credential stamp task: {e}")))?;
        let mut guard = self.cached.lock().await;
        let Some(entry) = guard.as_mut() else {
            return Ok(None);
        };
        match current {
            Ok(current) if current == entry.stamp => {
                entry.stamp_checked_at = tokio::time::Instant::now();
                drop(guard);
                tracing::debug!(cached_age_secs = age_secs, "token cache hit");
                Ok(Some(token))
            },
            Ok(_) => {
                tracing::info!("credentials changed on disk; discarding cached token");
                *guard = None;
                Ok(None)
            },
            Err(e) => {
                tracing::warn!(error = %e, "credential identity unreadable; cached token discarded");
                *guard = None;
                Ok(None)
            },
        }
    }
}

fn chain_error(e: &auth::ChainError) -> ForwardError {
    if e.is_terminal() {
        ForwardError::Auth(e.to_string())
    } else {
        ForwardError::AuthRetryable(e.to_string())
    }
}

fn sign_in_required_error() -> ForwardError {
    ForwardError::Auth(format!(
        "no JWT available — sign in via {} GUI",
        crate::brand::brand().app_name
    ))
}
