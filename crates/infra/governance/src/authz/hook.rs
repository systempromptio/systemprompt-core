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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_client::{GuardedClientConfig, guarded_client};

use super::audit::{AuthzAuditSink, AuthzSource, NullAuditSink};
use super::error::AuthzResult;
use super::types::{AuthzDecision, AuthzRequest, DenyReason};

/// `#[async_trait]`: this trait is consumed as `Arc<dyn AuthzDecisionHook>`
/// (see `authz::runtime`), so it must be `dyn`-compatible — native
/// `async fn` in traits is not yet object-safe.
///
/// `evaluate` decides an enforced request and records the decision to the
/// audit sink. `probe` decides a request the caller is only asking about —
/// the bridge profile admitting a model it would list — without writing an
/// audit row, and must return what `evaluate` would for the same request.
#[async_trait]
pub trait AuthzDecisionHook: Send + Sync + std::fmt::Debug {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision;

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision;
}

pub type SharedAuthzHook = Arc<dyn AuthzDecisionHook>;

#[derive(Debug, Clone)]
pub struct DenyAllHook {
    sink: Arc<dyn AuthzAuditSink>,
}

impl DenyAllHook {
    pub fn new(sink: Arc<dyn AuthzAuditSink>) -> Self {
        Self { sink }
    }

    pub fn null() -> Self {
        Self {
            sink: Arc::new(NullAuditSink),
        }
    }
}

#[async_trait]
impl AuthzDecisionHook for DenyAllHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        let decision = deny_all_decision();
        self.sink
            .record(&req, &decision, AuthzSource::DenyAllDefault)
            .await;
        decision
    }

    async fn probe(&self, _req: AuthzRequest) -> AuthzDecision {
        deny_all_decision()
    }
}

fn deny_all_decision() -> AuthzDecision {
    let policy = AuthzSource::DenyAllDefault.policy().to_owned();
    AuthzDecision::Deny {
        reason: DenyReason::HookUnavailable {
            policy: policy.clone(),
            detail: "no authz hook is configured; the default denies".to_owned(),
        },
        policy,
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

    async fn probe(&self, _req: AuthzRequest) -> AuthzDecision {
        AuthzDecision::Allow
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
        let client = guarded_client(&GuardedClientConfig::default().with_timeout(timeout))?;
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

    async fn fault(&self, req: &AuthzRequest, detail: String) -> AuthzDecision {
        let decision = webhook_fault_decision(detail);
        self.sink
            .record(req, &decision, AuthzSource::WebhookFault)
            .await;
        decision
    }
}

#[derive(Debug, thiserror::Error)]
enum WebhookFault {
    #[error("transport failure: {0}")]
    Transport(#[source] reqwest::Error),
    #[error("hook returned status {0}")]
    Status(u16),
    #[error("undecodable response: {0}")]
    Decode(#[source] reqwest::Error),
}

impl WebhookHook {
    async fn decide(&self, req: &AuthzRequest) -> Result<AuthzDecision, WebhookFault> {
        let response = self.client.post(&self.url).json(req).send().await;
        let response = match response {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    url = %self.url,
                    "authz hook transport failure",
                );
                return Err(WebhookFault::Transport(err));
            },
        };
        if !response.status().is_success() {
            tracing::warn!(
                status = response.status().as_u16(),
                url = %self.url,
                "authz hook returned non-success status",
            );
            return Err(WebhookFault::Status(response.status().as_u16()));
        }
        response.json::<AuthzDecision>().await.map_err(|err| {
            tracing::warn!(
                error = %err,
                url = %self.url,
                "authz hook response decode failure",
            );
            WebhookFault::Decode(err)
        })
    }
}

fn webhook_fault_decision(detail: String) -> AuthzDecision {
    let policy = AuthzSource::WebhookFault.policy().to_owned();
    AuthzDecision::Deny {
        reason: DenyReason::HookUnavailable {
            policy: policy.clone(),
            detail,
        },
        policy,
    }
}

// Why: the webhook is an extension's HTTP handler, so whether it writes its
// own audit row for a probe is the extension's call; core writes none.
#[async_trait]
impl AuthzDecisionHook for WebhookHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        match self.decide(&req).await {
            Ok(decision) => decision,
            Err(fault) => self.fault(&req, format!("{fault}")).await,
        }
    }

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
        self.decide(&req)
            .await
            .unwrap_or_else(|fault| webhook_fault_decision(format!("{fault}")))
    }
}
