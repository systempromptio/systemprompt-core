//! [`AuthzAuditSink`] backed by [`GovernanceDecisionRepository`].
//!
//! Failure to insert is logged via `tracing::error!` but never propagates —
//! the calling hook has already decided; losing the audit row must not flip
//! a deny to an allow.

use async_trait::async_trait;
use systemprompt_identifiers::Actor;

use super::repository::{GovernanceDecisionRecord, GovernanceDecisionRepository};
use super::{AuthzAuditSink, AuthzSource};
use crate::authz::types::{AuthzDecision, AuthzRequest, DecisionTag};

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
        let decision_tag = DecisionTag::from(decision);
        let reason_str = match decision {
            AuthzDecision::Allow => String::new(),
            AuthzDecision::Deny { reason, .. } => reason.to_string(),
        };
        let entity_type = req.entity.kind().as_str();
        let entity_id = req.entity.id_str();
        let evaluated = serde_json::json!({
            "entity_type": entity_type,
            "entity_id": entity_id,
            "trace_id": req.trace_id.as_str(),
            "roles": req.roles,
            "department": req.department,
            "context": req.context,
            "source": format!("{:?}", source),
        });
        let actor = Actor::user(req.user_id.clone());
        let record = GovernanceDecisionRecord {
            id: &id,
            actor: &actor,
            session_id: req.trace_id.as_str(),
            tool_name: entity_id,
            agent_id: None,
            agent_scope: entity_type,
            decision: decision_tag,
            policy: source.policy(),
            reason: &reason_str,
            evaluated_rules: &evaluated,
            plugin_id: None,
            act_chain: &req.act_chain,
        };
        if let Err(err) = self.repo.insert(&record).await {
            tracing::error!(
                error = %err,
                policy = source.policy(),
                entity_type = %entity_type,
                entity_id = %entity_id,
                "failed to record core-side authz decision"
            );
        }
    }
}
