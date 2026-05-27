//! Core `AuthzDecisionHook` wrapping the in-process [`super::resolver`].
//!
//! `RuleBasedHook` is the canonical RBAC layer: it loads
//! `access_control_rules` for the request's entity, runs the sync resolver
//! over them, and emits an `AuthzDecision`. Exposed as a hook so extensions
//! can compose it explicitly with their own ABAC predicates via
//! [`super::CompositeAuthzHook`]:
//!
//! ```ignore
//! let composite = CompositeAuthzHook::new(vec![
//!     Arc::new(RuleBasedHook::new(pool.clone(), sink.clone())),
//!     Arc::new(MyAbacHook::new(...)),
//! ]);
//! ```
//!
//! Put `RuleBasedHook` first so a coarse-grained RBAC reject short-circuits
//! the chain before any per-attribute lookup runs.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use super::audit::{AuthzAuditSink, AuthzSource};
use super::hook::AuthzDecisionHook;
use super::repository::AccessControlRepository;
use super::resolver::{ResolveInput, resolve};
use super::types::{AuthzDecision, AuthzRequest, Decision, DenyReason};

#[derive(Debug, Clone)]
pub struct RuleBasedHook {
    repo: AccessControlRepository,
    sink: Arc<dyn AuthzAuditSink>,
}

impl RuleBasedHook {
    #[must_use]
    pub fn new(pool: Arc<PgPool>, sink: Arc<dyn AuthzAuditSink>) -> Self {
        Self {
            repo: AccessControlRepository::from_pool(pool),
            sink,
        }
    }

    async fn fault(&self, req: &AuthzRequest, detail: &str) -> AuthzDecision {
        let policy = AuthzSource::RuleBased.policy().to_owned();
        let decision = AuthzDecision::Deny {
            reason: DenyReason::HookUnavailable {
                policy: policy.clone(),
            },
            policy,
        };
        tracing::warn!(
            entity = %req.entity,
            user_id = %req.user_id,
            error = %detail,
            "rule-based authz hook fault",
        );
        self.sink
            .record(req, &decision, AuthzSource::RuleBased)
            .await;
        decision
    }
}

#[async_trait]
impl AuthzDecisionHook for RuleBasedHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        let kind = req.entity.kind();
        let id = req.entity.id_str();

        let entity = match self.repo.get_entity(kind, id).await {
            Ok(row) => row,
            Err(err) => return self.fault(&req, &err.to_string()).await,
        };
        let rules = match self.repo.list_rules_for_entity(kind, id).await {
            Ok(rules) => rules,
            Err(err) => return self.fault(&req, &err.to_string()).await,
        };

        let decision = resolve(ResolveInput {
            entity: &req.entity,
            rules: &rules,
            user_id: &req.user_id,
            user_roles: &req.roles,
            default_included: entity.map(|e| e.default_included),
        });

        let policy = AuthzSource::RuleBased.policy().to_owned();
        let authz_decision = match decision {
            Decision::Allow { .. } => AuthzDecision::Allow,
            Decision::Deny { reason } => AuthzDecision::Deny { reason, policy },
        };
        self.sink
            .record(&req, &authz_decision, AuthzSource::RuleBased)
            .await;
        authz_decision
    }
}
