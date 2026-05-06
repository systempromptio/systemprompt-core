//! [`AuthzAuditSink`] backed by [`GovernanceDecisionRepository`].
//!
//! Failure to insert is logged via `tracing::error!` but never propagates —
//! the calling hook has already decided; losing the audit row must not flip
//! a deny to an allow.

use async_trait::async_trait;

use super::repository::{GovernanceDecisionRecord, GovernanceDecisionRepository};
use super::{AuthzAuditSink, AuthzSource};
use crate::authz::types::{AuthzDecision, AuthzRequest};

#[derive(Debug, Clone)]
pub struct DbAuditSink {
    repo: GovernanceDecisionRepository,
}

impl DbAuditSink {
    pub const fn new(repo: GovernanceDecisionRepository) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl AuthzAuditSink for DbAuditSink {
    async fn record(&self, req: &AuthzRequest, decision: &AuthzDecision, source: AuthzSource) {
        let id = uuid::Uuid::new_v4().to_string();
        let (decision_str, reason_str) = match decision {
            AuthzDecision::Allow => ("allow", String::new()),
            AuthzDecision::Deny { reason, .. } => ("deny", reason.clone()),
        };
        let entity_type = req.entity_type.as_str();
        let evaluated = serde_json::json!({
            "entity_type": entity_type,
            "entity_id": req.entity_id,
            "trace_id": req.trace_id.as_str(),
            "roles": req.roles,
            "department": req.department,
            "context": req.context,
            "source": format!("{:?}", source),
        });
        let record = GovernanceDecisionRecord {
            id: &id,
            user_id: req.user_id.as_str(),
            session_id: req.trace_id.as_str(),
            tool_name: &req.entity_id,
            agent_id: None,
            agent_scope: entity_type,
            decision: decision_str,
            policy: source.policy(),
            reason: &reason_str,
            evaluated_rules: &evaluated,
            plugin_id: None,
        };
        if let Err(err) = self.repo.insert(&record).await {
            tracing::error!(
                error = %err,
                policy = source.policy(),
                entity_type = %entity_type,
                entity_id = %req.entity_id,
                "failed to record core-side authz decision"
            );
        }
    }
}
