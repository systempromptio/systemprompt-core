//! Audit blob for governed-call decisions.
//!
//! [`DecisionAudit`] is the typed shape serialized whole into
//! `governance_decisions.evaluated_rules`; the flat columns (`decision`,
//! `reason`, `policy`) are derived from it by [`record_decision`], which
//! delegates to the canonical
//! [`insert_governance_decision`] writer. The serialized shape is a persisted
//! contract rendered by dashboards
//! — field renames here are schema changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use sqlx::PgPool;
use systemprompt_identifiers::{
    Actor, AgentId, ClientId, ContextId, PluginId, PolicyId, SessionId, UserId,
};

use super::types::AccessScope;
use crate::authz::types::{Decision, DecisionTag};
use crate::authz::{GovernanceDecisionRecord, insert_governance_decision};

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum ChainEntryResult {
    Pass,
    Fail,
    /// The policy found what it would normally refuse, but runs in
    /// `mode: warn`, so the chain continued. Kept distinct from `Fail` so a
    /// warn-mode installation cannot be misread as an enforcing one.
    Warn,
    Disabled,
    Skip,
    Hold,
}

/// One traced chain entry: which policy, what it decided, and what it cost.
#[derive(Debug, Serialize, Clone)]
pub struct ChainEntryOutcome {
    pub policy_id: PolicyId,
    #[serde(flatten)]
    pub result: ChainEntryResult,
    pub detail: String,
    pub duration_ms: f64,
}

/// Who the decision was made for, as verified from the credential.
///
/// `agent_id` is a verified delegate identity and lands in the `agent_id`
/// column; `claimed` is whatever the caller *said* about itself (a hook
/// payload's subagent id, for instance) and is kept in the audit blob only —
/// it is never an input to a decision and never written to an identity
/// column.
#[derive(Debug, Serialize, Clone)]
pub struct PrincipalSnapshot {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub agent_session: Option<SessionId>,
    pub agent_id: Option<AgentId>,
    pub agent_scope: AccessScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<ClientId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed: Option<ClaimedAgent>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ClaimedAgent {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AuditTarget {
    pub tool_name: String,
    pub plugin_id: Option<PluginId>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ApproverStamp {
    pub user_id: UserId,
    pub username: String,
    pub decided_at: chrono::DateTime<chrono::Utc>,
    pub action: &'static str,
}

/// Whether an audit row is the first judgement of a call or a later
/// enforcement point re-verifying it.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditOrigin {
    Governed,
    Reverified,
}

#[derive(Debug, Serialize, Clone)]
pub struct DecisionAudit {
    pub id: String,
    pub call_id: String,
    pub origin: AuditOrigin,
    pub decision: Decision,
    pub principal: PrincipalSnapshot,
    pub target: AuditTarget,
    pub chain: Vec<ChainEntryOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approver: Option<ApproverStamp>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub act_chain: Vec<Actor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
    // Why: persisted to the `trace_id` column so the trace explorer joins on
    // a real key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

// Why: an allow because nothing ran and an allow because everything passed are
// the same `Decision`, and the flat `policy` column is what operational queries
// filter on. Collapsing both to `default_allow` would make an unguarded
// installation indistinguishable from a healthy one.
fn allow_policy_label(chain: &[ChainEntryOutcome]) -> &'static str {
    if !chain.is_empty() && chain.iter().all(|e| e.result == ChainEntryResult::Disabled) {
        return "governance_disabled";
    }
    "default_allow"
}

// Why: by the same argument, an allow because a *human authorised it* is a
// third thing again, and the one an audit reader most needs to tell apart. It
// carries an approver, so the policy that held it is named rather than
// collapsed into `default_allow` — otherwise an approved call is reported as
// though nothing enforced it.
fn approved_policy_label(audit: &DecisionAudit) -> Option<String> {
    audit.approver.as_ref()?;
    audit
        .chain
        .iter()
        .find(|e| e.result == ChainEntryResult::Pass)
        .map(|e| e.policy_id.as_str().to_owned())
}

pub async fn record_decision(pool: &PgPool, audit: &DecisionAudit) -> Result<(), sqlx::Error> {
    let actor = Actor::from_tool_name(
        audit.principal.user_id.clone(),
        audit.principal.agent_id.as_ref().map(AgentId::as_str),
        &audit.target.tool_name,
    );
    let (decision_tag, reason_str, policy_str) = match &audit.decision {
        Decision::Allow { .. } => (
            DecisionTag::Allow,
            String::new(),
            approved_policy_label(audit)
                .unwrap_or_else(|| allow_policy_label(&audit.chain).to_owned()),
        ),
        Decision::Deny { reason } => {
            let policy_str = audit
                .chain
                .iter()
                .find(|e| e.result == ChainEntryResult::Fail)
                .map_or_else(|| "unknown".to_owned(), |e| e.policy_id.as_str().to_owned());
            (DecisionTag::Deny, reason.to_string(), policy_str)
        },
        // Why: the `policy` column names the first policy that warned, not
        // `default_allow`. A warn row whose policy read `default_allow` would
        // be useless to the report the mode exists to feed.
        Decision::Warn { reason } => {
            let policy_str = audit
                .chain
                .iter()
                .find(|e| e.result == ChainEntryResult::Warn)
                .map_or_else(|| "unknown".to_owned(), |e| e.policy_id.as_str().to_owned());
            (DecisionTag::Warn, reason.to_string(), policy_str)
        },
        Decision::Pending { reason } => {
            let policy_str = audit
                .chain
                .iter()
                .find(|e| e.result == ChainEntryResult::Hold)
                .map_or_else(|| "unknown".to_owned(), |e| e.policy_id.as_str().to_owned());
            (DecisionTag::Pending, reason.to_string(), policy_str)
        },
    };
    let evaluated_rules = serde_json::to_value(audit).unwrap_or_else(|e| {
        tracing::error!(
            error = %e,
            tool_name = %audit.target.tool_name,
            "could not serialise the governance evaluation trace; recording the decision \
             without it"
        );
        serde_json::Value::Null
    });

    let context_id = audit
        .context_id
        .as_deref()
        .and_then(|s| ContextId::try_new(s).ok())
        .unwrap_or_else(|| ContextId::derived_from_session(&audit.principal.session_id));
    let record = GovernanceDecisionRecord {
        id: &audit.id,
        actor: &actor,
        session_id: audit.principal.session_id.as_str(),
        tool_name: &audit.target.tool_name,
        agent_id: audit.principal.agent_id.as_ref().map(AgentId::as_str),
        agent_scope: Some(audit.principal.agent_scope),
        decision: decision_tag,
        policy: &policy_str,
        reason: &reason_str,
        evaluated_rules: &evaluated_rules,
        plugin_id: audit.target.plugin_id.as_ref().map(PluginId::as_str),
        act_chain: &audit.act_chain,
        context_id: context_id.as_str(),
        task_id: None,
        trace_id: audit.trace_id.as_deref(),
        client_id: audit.principal.client_id.as_ref().map(ClientId::as_str),
    };

    insert_governance_decision(pool, &record).await
}
