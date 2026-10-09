//! Admission writes held on the audit until the request commits.
//!
//! `open` and the pre-dispatch stages describe the request row, its payload,
//! messages, route, prepared body, request-phase findings and governance
//! decisions; nothing is
//! written until [`GatewayAudit::commit_admission`] sends them in one
//! transaction, right before the upstream call or on the first failure path.
//! Holding the rows in memory rather than an open transaction keeps no pooled
//! connection pinned while quota and extension guards acquire their own.
//! Once committed, the setters fall back to their direct per-field writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use systemprompt_ai::models::AiRequestRecord;
use systemprompt_ai::repository::{
    AdmissionWrite, InsertSafetyFinding, PreparedDigest, RequestMessageRow, UpsertPayloadParams,
};
use systemprompt_security::policy::DecisionAudit;

use super::GatewayAudit;
use super::payload::PayloadCapture;
use crate::error::{GatewayAuditError, GatewayAuditResult as Result};

pub(super) struct PendingAdmission {
    pub(super) record: AiRequestRecord,
    pub(super) capture: PayloadCapture,
    // JSON: client tool definitions, stored verbatim as JSONB.
    pub(super) offered_tools: Option<Value>,
    pub(super) messages: Vec<(&'static str, String)>,
    route_match: Option<String>,
    served_provider: Option<String>,
    system_prompt_override: Option<String>,
    // JSON: client tool definitions, stored verbatim as JSONB.
    prepared: Option<(String, Option<Value>)>,
    findings: Vec<PendingFinding>,
    decisions: Vec<DecisionAudit>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingFinding {
    pub(crate) phase: String,
    pub(crate) severity: String,
    pub(crate) category: String,
    pub(crate) scanner: String,
    pub(crate) excerpt: Option<String>,
    pub(crate) blocked: bool,
}

impl PendingAdmission {
    pub(super) const fn new(
        record: AiRequestRecord,
        capture: PayloadCapture,
        // JSON: client tool definitions, stored verbatim as JSONB.
        offered_tools: Option<Value>,
        messages: Vec<(&'static str, String)>,
    ) -> Self {
        Self {
            record,
            capture,
            offered_tools,
            messages,
            route_match: None,
            served_provider: None,
            system_prompt_override: None,
            prepared: None,
            findings: Vec::new(),
            decisions: Vec::new(),
        }
    }
}

impl GatewayAudit {
    fn stash(&self, apply: impl FnOnce(&mut PendingAdmission)) -> bool {
        let Ok(mut slot) = self.admission.lock() else {
            return false;
        };
        slot.as_mut().map(apply).is_some()
    }

    pub(super) fn stash_route_match(&self, descriptor: &str) -> bool {
        self.stash(|p| p.route_match = Some(descriptor.to_owned()))
    }

    pub(super) fn stash_served_provider(&self, provider: &str) -> bool {
        self.stash(|p| p.served_provider = Some(provider.to_owned()))
    }

    pub(super) fn stash_system_prompt_override(&self, descriptor: &str) -> bool {
        self.stash(|p| p.system_prompt_override = Some(descriptor.to_owned()))
    }

    // JSON: client tool definitions, stored verbatim as JSONB.
    pub(super) fn stash_prepared(&self, sha256: &str, tools: Option<&Value>) -> bool {
        self.stash(|p| p.prepared = Some((sha256.to_owned(), tools.cloned())))
    }

    pub(crate) fn stash_request_findings(&self, findings: &[PendingFinding]) -> bool {
        self.stash(|p| p.findings.extend_from_slice(findings))
    }

    pub(crate) fn stash_decision(&self, decision: DecisionAudit) -> Option<DecisionAudit> {
        let Ok(mut slot) = self.admission.lock() else {
            return Some(decision);
        };
        match slot.as_mut() {
            Some(pending) => {
                pending.decisions.push(decision);
                None
            },
            None => Some(decision),
        }
    }

    pub async fn commit_admission(&self) -> Result<()> {
        let pending = self
            .admission
            .lock()
            .map_err(|_poisoned| GatewayAuditError::Invariant("admission slot poisoned"))?
            .take();
        let Some(pending) = pending else {
            return Ok(());
        };
        self.write_admission(&pending).await?;
        let lease = super::journal::reserve(
            &self.settlement.journal,
            super::journal::Receipt::pending(
                self.ctx.ai_request_id.clone(),
                self.ctx.user_id.clone(),
                self.ctx.session_id.clone(),
            ),
        )
        .await?;
        self.journal_lease
            .set(lease)
            .map_err(|_existing_lease| GatewayAuditError::Invariant("Audit already admitted"))?;
        Ok(())
    }

    async fn write_admission(&self, pending: &PendingAdmission) -> Result<()> {
        let messages: Vec<RequestMessageRow<'_>> = pending
            .messages
            .iter()
            .zip(0i32..)
            .map(|((role, content), sequence_number)| RequestMessageRow {
                role,
                content,
                sequence_number,
            })
            .collect();
        let findings: Vec<InsertSafetyFinding<'_>> = pending
            .findings
            .iter()
            .map(|f| InsertSafetyFinding {
                ai_request_id: &self.ctx.ai_request_id,
                phase: &f.phase,
                severity: &f.severity,
                category: &f.category,
                scanner: &f.scanner,
                excerpt: f.excerpt.as_deref(),
                blocked: f.blocked,
            })
            .collect();
        let capture = &pending.capture;
        self.requests
            .admit(AdmissionWrite {
                id: &self.ctx.ai_request_id,
                record: &pending.record,
                evidence: &self.ctx.evidence,
                payload: UpsertPayloadParams {
                    body: capture.json.as_ref(),
                    excerpt: capture.excerpt.as_deref(),
                    truncated: capture.truncated,
                    bytes: Some(capture.byte_len),
                    sha256: Some(&capture.sha256),
                },
                offered_tools: pending.offered_tools.as_ref(),
                messages: &messages,
                route_match: pending.route_match.as_deref(),
                served_provider: pending.served_provider.as_deref(),
                system_prompt_override: pending.system_prompt_override.as_deref(),
                prepared: pending
                    .prepared
                    .as_ref()
                    .map(|(sha256, tools)| PreparedDigest {
                        sha256,
                        tools: tools.as_ref(),
                    }),
                findings: &findings,
                decisions: &pending.decisions,
            })
            .await?;
        Ok(())
    }
}
