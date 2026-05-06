//! Audit sink for authorization decisions.
//!
//! Every decision made *inside core* (webhook fault, default deny,
//! unrestricted allow) flows through an [`AuthzAuditSink`] so it lands in the
//! same `governance_decisions` table the extension's `POST /govern/authz`
//! handler writes to. Successful webhook round-trips are audited by the
//! extension itself (single writer per code path); core's sink only records
//! decisions the extension never sees.
//!
//! [`NullAuditSink`] is the bootstrap default — it exists so unit tests and
//! pre-database bootstrap stages can install hooks without a `DbPool`.
//! Production replaces it with [`DbAuditSink`] once the database is available.

mod db_sink;
mod repository;

use async_trait::async_trait;

use super::types::{AuthzDecision, AuthzRequest};

pub use db_sink::DbAuditSink;
pub use repository::{
    GovernanceDecisionRecord, GovernanceDecisionRepository, insert_governance_decision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthzSource {
    WebhookFault,
    DenyAllDefault,
    AllowAllUnrestricted,
}

impl AuthzSource {
    pub const fn policy(self) -> &'static str {
        match self {
            Self::WebhookFault => "authz_hook_fault",
            Self::DenyAllDefault => "authz_default_deny",
            Self::AllowAllUnrestricted => "authz_unrestricted",
        }
    }
}

#[async_trait]
pub trait AuthzAuditSink: Send + Sync + std::fmt::Debug {
    async fn record(&self, req: &AuthzRequest, decision: &AuthzDecision, source: AuthzSource);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NullAuditSink;

#[async_trait]
impl AuthzAuditSink for NullAuditSink {
    async fn record(&self, _req: &AuthzRequest, _decision: &AuthzDecision, _source: AuthzSource) {}
}
