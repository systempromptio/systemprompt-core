//! The scan stage: request safety scanners judge the governed request, a
//! blocking finding refuses it, findings in `safety.redact_categories` are
//! rewritten in the forwarded body, and the findings are persisted after
//! redaction so a redacted category is stored as its marker.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiRequestId;

use super::super::super::audit::GatewayAudit;
use super::super::finalize::{
    persist_request_findings, request_finding_blocks, run_request_safety_scan,
};
use super::super::{DispatchError, SafetyBlocked};
use super::recovery::redact_findings;
use super::{GovernedDispatch, ScannedDispatch};
use crate::policies::SafetyConfig;

impl ScannedDispatch {
    pub(in crate::service) async fn enforce(
        governed: GovernedDispatch,
        repos: &super::super::super::GatewayRepositories,
        ai_request_id: &AiRequestId,
        safety: &SafetyConfig,
        audit: &GatewayAudit,
    ) -> Result<Self, DispatchError> {
        let GovernedDispatch(mut prepared) = governed;
        let findings = run_request_safety_scan(&prepared.request, safety).await;
        let blocked = findings
            .iter()
            .find(|f| request_finding_blocks(f, safety))
            .map(|f| (f.category.clone(), f.scanner));
        let refusal = match blocked {
            Some((category, scanner)) => Some((category, scanner, "")),
            None => match redact_findings(
                &mut prepared.request,
                &mut prepared.body,
                &findings,
                &safety.redact_categories,
            ) {
                Ok(None) => None,
                Ok(Some(report)) => {
                    audit.set_prepared_body_digest(&prepared.body.bytes).await;
                    tracing::info!(
                        ai_request_id = %ai_request_id,
                        parts = report.parts,
                        categories = ?report.categories,
                        "Gateway redacted safety findings in the forwarded request"
                    );
                    None
                },
                Err(unredactable) => {
                    Some((unredactable.category, "redaction", " could not be redacted"))
                },
            },
        };
        persist_request_findings(&repos.safety_findings, ai_request_id, &findings, safety).await;
        let Some((category, scanner, detail)) = refusal else {
            return Ok(Self(prepared));
        };
        let msg = format!("request blocked by safety policy: category '{category}'{detail}");
        tracing::warn!(
            ai_request_id = %ai_request_id,
            category = %category,
            scanner = %scanner,
            "Gateway blocked request by safety policy"
        );
        if let Err(e) = audit.fail(&msg).await {
            tracing::warn!(error = %e, "safety-block audit fail failed");
        }
        Err(DispatchError::recorded(SafetyBlocked {
            category,
            message: msg,
        }))
    }
}
