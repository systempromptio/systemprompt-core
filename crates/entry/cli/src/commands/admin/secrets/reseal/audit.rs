//! The decision rows a re-seal run writes and the report of a member's sealed
//! files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_extension::SealedFiles;
use systemprompt_governance::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
};
use systemprompt_identifiers::{Actor, ContextId, ExtensionId};
use systemprompt_manifest::services::SystemAdmin;

use super::{ColumnReport, RESEAL_POLICY};

pub fn files_report(files: &dyn SealedFiles, apply: bool) -> Result<ColumnReport> {
    let report = files.reseal(apply).with_context(|| {
        format!(
            "re-sealing the {} files of {}",
            files.label(),
            files.owner()
        )
    })?;
    Ok(ColumnReport {
        table: files.label().to_owned(),
        column: "segments".to_owned(),
        total: report.total,
        member_sealed: report.current,
        resealed: report.resealed,
        undecodable: report.undecodable,
        unkeyed: 0,
        unwritten: 0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditPhase {
    Planned,
    Applied,
}

impl AuditPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Applied => "applied",
        }
    }
}

pub(super) async fn audit(
    repository: &GovernanceDecisionRepository,
    operator: &SystemAdmin,
    member: &ExtensionId,
    phase: AuditPhase,
    columns: &[ColumnReport],
) -> Result<()> {
    let actor = Actor::job(operator.id().clone(), "cli:admin secrets reseal");
    let context = ContextId::generate();
    let id = format!("member-reseal-{}-{}", phase.as_str(), context.as_str());
    let reason = match phase {
        AuditPhase::Planned => format!("core is re-sealing the declared ciphertext of {member}"),
        AuditPhase::Applied => format!("core re-sealed the declared ciphertext of {member}"),
    };
    let rules = serde_json::json!({
        "member": member.as_str(),
        "operator": operator.id().as_str(),
        "phase": phase.as_str(),
        "columns": columns,
    });
    repository
        .insert(&GovernanceDecisionRecord {
            id: &id,
            actor: &actor,
            session_id: None,
            tool_name: RESEAL_POLICY,
            agent_id: None,
            agent_scope: None,
            decision: DecisionTag::Allow,
            policy: RESEAL_POLICY,
            reason: &reason,
            evaluated_rules: &rules,
            plugin_id: None,
            act_chain: &[],
            context_id: &context,
            task_id: None,
            trace_id: None,
            client_id: None,
            tool_use_id: None,
        })
        .await
        .context("the re-seal could not be audited, so nothing was written")
}
