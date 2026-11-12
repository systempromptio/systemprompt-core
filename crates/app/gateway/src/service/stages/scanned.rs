//! The scan stage: request safety scanners judge the governed request, a
//! blocking finding refuses it, findings in `safety.redact_categories` are
//! rewritten in the forwarded body, and the findings are persisted after
//! redaction so a redacted category is stored as its marker. A hop whose
//! policy does not redact refuses the request such a finding lands on.
//!
//! The scanners read the string leaves of the prepared body, the exact bytes
//! the upstream receives, so content the inbound parser does not model
//! (unknown block types, unmodelled fields) is judged on every wire. When that
//! surface is cut short by its budget under an enforcing scanner set, the
//! request is refused rather than forwarded partly unread.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiRequestId;
use systemprompt_wire::inspect;

use super::super::super::audit::GatewayAudit;
use super::super::finalize::{
    persist_request_findings, request_finding_blocks, request_finding_rows, run_request_safety_scan,
};
use super::super::{DispatchError, SafetyBlocked};
use super::recovery::{inspection_budget, redact_findings, redactable};
use super::{GovernedDispatch, ScannedDispatch};
use crate::policies::SafetyConfig;
use crate::registry::SafetyScannerRegistry;

pub(in crate::service) struct SafetyScan<'a> {
    pub scanners: &'a SafetyScannerRegistry,
    pub safety: &'a SafetyConfig,
}

impl ScannedDispatch {
    pub(in crate::service) async fn enforce(
        governed: GovernedDispatch,
        repos: &super::super::super::GatewayRepositories,
        scan: SafetyScan<'_>,
        ai_request_id: &AiRequestId,
        audit: &GatewayAudit,
    ) -> Result<Self, DispatchError> {
        let SafetyScan { scanners, safety } = scan;
        let GovernedDispatch(mut prepared) = governed;
        prepared.request.forwarded_surface =
            inspect::string_leaves(&prepared.body.bytes, inspection_budget());
        if enforcing(safety) && surface_incomplete(&prepared) {
            return Err(refuse(
                audit,
                ai_request_id,
                CATEGORY_INSPECTION_INCOMPLETE.to_owned(),
                "inspection",
                " because the forwarded body exceeds the inspection budget",
            )
            .await);
        }
        let findings = run_request_safety_scan(scanners, &prepared.request, safety).await;
        let blocked = findings
            .iter()
            .find(|f| request_finding_blocks(f, safety))
            .map(|f| (f.category.clone(), f.scanner));
        let refusal = match blocked {
            Some((category, scanner)) => Some((category, scanner, "")),
            None if !prepared.policy.redacts() => redactable(&findings, &safety.redact_categories)
                .next()
                .map(|finding| {
                    (
                        finding.category.clone(),
                        "redaction",
                        " is not redacted on a request sent as its client built it",
                    )
                }),
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
        if !audit.stash_request_findings(&request_finding_rows(&findings, safety)) {
            persist_request_findings(&repos.safety_findings, ai_request_id, &findings, safety)
                .await;
        }
        let Some((category, scanner, detail)) = refusal else {
            return Ok(Self(prepared));
        };
        Err(refuse(audit, ai_request_id, category, scanner, detail).await)
    }
}

const CATEGORY_INSPECTION_INCOMPLETE: &str = "inspection_incomplete";

const fn enforcing(safety: &SafetyConfig) -> bool {
    !safety.mode.is_warn() && !safety.scanners.is_empty()
}

fn surface_incomplete(prepared: &super::PreparedDispatch) -> bool {
    let surface = &prepared.request.forwarded_surface;
    surface.truncated()
        || (surface.is_empty()
            && serde_json::from_slice::<serde_json::Value>(&prepared.body.bytes).is_err())
}

async fn refuse(
    audit: &GatewayAudit,
    ai_request_id: &AiRequestId,
    category: String,
    scanner: &str,
    detail: &str,
) -> DispatchError {
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
    DispatchError::recorded(SafetyBlocked {
        category,
        message: msg,
    })
}
