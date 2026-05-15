//! Authorization decision hooks.
//!
//! Core fires [`AuthzDecisionHook::evaluate`] from the gateway and MCP
//! enforcement sites. Three implementations:
//!
//! - [`WebhookHook`] — production. POSTs to an extension HTTP handler (e.g. the
//!   template's `POST /govern/authz`). Any transport error, non-2xx, decode
//!   failure, or timeout **denies** the request and records the fault to the
//!   audit sink. There is no fail-open mode.
//! - [`DenyAllHook`] — bootstrap default and `mode: disabled`. Denies every
//!   request and records to the audit sink so outages remain observable.
//! - [`AllowAllHook`] — TEST/DEV ONLY. Installed only when the operator passes
//!   the explicit `unrestricted` acknowledgement in the profile. Allows every
//!   request; logs an `ERROR` line at boot and writes an audit row per call so
//!   unrestricted operation is never silent.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::audit::{AuthzAuditSink, AuthzSource, NullAuditSink};
use super::error::AuthzResult;
use super::types::{AuthzDecision, AuthzRequest};

/// `#[async_trait]`: this trait is consumed as `Arc<dyn AuthzDecisionHook>`
/// (see `authz::runtime`), so it must be `dyn`-compatible — native
/// `async fn` in traits is not yet object-safe.
#[async_trait]
pub trait AuthzDecisionHook: Send + Sync + std::fmt::Debug {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision;
}

#[derive(Debug, Clone)]
pub struct DenyAllHook {
    sink: Arc<dyn AuthzAuditSink>,
}

impl DenyAllHook {
    pub fn new(sink: Arc<dyn AuthzAuditSink>) -> Self {
        Self { sink }
    }

    /// Construct a `DenyAllHook` with no audit sink. Intended for tests and
    /// pre-database bootstrap; production paths should always pass a real
    /// sink so denies during outages are observable.
    pub fn null() -> Self {
        Self {
            sink: Arc::new(NullAuditSink),
        }
    }
}

#[async_trait]
impl AuthzDecisionHook for DenyAllHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        let decision = AuthzDecision::Deny {
            reason: "no authz hook configured".into(),
            policy: AuthzSource::DenyAllDefault.policy().to_string(),
        };
        self.sink
            .record(&req, &decision, AuthzSource::DenyAllDefault)
            .await;
        decision
    }
}

#[derive(Debug, Clone)]
pub struct AllowAllHook {
    sink: Arc<dyn AuthzAuditSink>,
}

impl AllowAllHook {
    pub fn new(sink: Arc<dyn AuthzAuditSink>) -> Self {
        Self { sink }
    }

    /// Construct an `AllowAllHook` with no audit sink. Tests only — production
    /// installs only happen via the explicit unrestricted opt-in path which
    /// always wires a real sink.
    pub fn null() -> Self {
        Self {
            sink: Arc::new(NullAuditSink),
        }
    }
}

#[async_trait]
impl AuthzDecisionHook for AllowAllHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        let decision = AuthzDecision::Allow;
        self.sink
            .record(&req, &decision, AuthzSource::AllowAllUnrestricted)
            .await;
        decision
    }
}

#[derive(Debug, Clone)]
pub struct WebhookHook {
    url: String,
    timeout: Duration,
    client: reqwest::Client,
    sink: Arc<dyn AuthzAuditSink>,
}

impl WebhookHook {
    pub fn new(url: String, timeout: Duration, sink: Arc<dyn AuthzAuditSink>) -> AuthzResult<Self> {
        let client = reqwest::Client::builder().timeout(timeout).build()?;
        Ok(Self {
            url,
            timeout,
            client,
            sink,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    async fn fault(&self, req: &AuthzRequest) -> AuthzDecision {
        let decision = AuthzDecision::Deny {
            reason: "authz hook unreachable".into(),
            policy: AuthzSource::WebhookFault.policy().to_string(),
        };
        self.sink
            .record(req, &decision, AuthzSource::WebhookFault)
            .await;
        decision
    }
}

#[async_trait]
impl AuthzDecisionHook for WebhookHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        let response = self.client.post(&self.url).json(&req).send().await;
        let response = match response {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    url = %self.url,
                    "authz hook transport failure",
                );
                return self.fault(&req).await;
            },
        };
        if !response.status().is_success() {
            tracing::warn!(
                status = response.status().as_u16(),
                url = %self.url,
                "authz hook returned non-success status",
            );
            return self.fault(&req).await;
        }
        match response.json::<AuthzDecision>().await {
            Ok(decision) => decision,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    url = %self.url,
                    "authz hook response decode failure",
                );
                self.fault(&req).await
            },
        }
    }
}
