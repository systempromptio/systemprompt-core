//! Side-effect contracts the SSO driver calls out to: the sign-in audit sink
//! and the assertion replay guard.
//!
//! Both are dispatched as `Arc<dyn …>` (several sinks fan out from one
//! driver), so they use `#[async_trait]`; native `async fn` in traits is not
//! yet `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;
use std::net::IpAddr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

use super::{GatedIdentity, SsoError};

/// A completed sign-in: the identity was admitted and a session minted.
#[derive(Debug, Clone, Copy)]
pub struct SsoLoginSuccess<'a> {
    pub provider_id: &'a str,
    pub issuer: &'a str,
    pub user_id: &'a UserId,
    pub session_id: &'a str,
    pub identity: &'a GatedIdentity,
    pub created: bool,
    pub ip: Option<IpAddr>,
}

/// A refused sign-in, with the audit reason and whatever identity was known
/// when it was refused.
#[derive(Debug, Clone, Copy)]
pub struct SsoLoginRefused<'a> {
    pub provider_id: &'a str,
    pub issuer: &'a str,
    pub reason: &'static str,
    pub email: Option<&'a str>,
    pub external_sub: Option<&'a str>,
    pub ip: Option<IpAddr>,
}

/// Receives every SSO sign-in outcome. A sink must not fail the sign-in: it
/// logs its own errors.
#[async_trait]
pub trait AuthEventSink: Send + Sync + Debug {
    async fn on_login_success(&self, event: &SsoLoginSuccess<'_>);

    async fn on_login_refused(&self, event: &SsoLoginRefused<'_>);

    async fn on_logout(&self, _user_id: &UserId, _session_id: &str) {}
}

/// Single-use marker for IdP assertions, authoritative across instances.
#[async_trait]
pub trait ReplayGuard: Send + Sync + Debug {
    /// `true` the first time `(scope, id)` is seen, `false` on every replay.
    async fn claim(
        &self,
        scope: &str,
        id: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<bool, SsoError>;
}
