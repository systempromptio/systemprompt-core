//! Staged dispatch pipeline: `PreparedDispatch` → `GovernedDispatch` →
//! `ScannedDispatch` → upstream send.
//!
//! Each stage owns the request by value and is only constructible from the
//! previous one, so the ordering the gateway's audit trail depends on —
//! build the exact wire payload, then govern it, then scan it, then send it —
//! is enforced by the types rather than by call-site discipline. Governance
//! ahead of the scanner plane also keeps first-deny-wins across both: a
//! denied request produces exactly one audit row and one 403.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod governance;
mod outbound;

use bytes::Bytes;
use systemprompt_ai::SafetyConfig;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::services::GatewayConfig;
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::inspect;
use systemprompt_security::authz::types::Decision;
use systemprompt_security::policy::ChainEntryResult;

pub(in crate::services::gateway::service) use self::governance::record_quota_warning;
use self::governance::{PromptEvaluation, evaluate_prompt, record_governance_decision};
use self::outbound::{CtxParts, audit_upstream_failure, outbound_ctx, strip_caller_identity};
use super::super::audit::{GatewayAudit, GatewayRequestContext};
use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::inbound::InboundAdapter;
use super::super::protocol::outbound::{OutboundOutcome, PreparedBody};
use super::finalize::{
    apply_system_prompt_override, request_finding_blocks, run_request_safety_scan,
};
use super::resolve::ResolvedUpstream;
use super::{DispatchError, GovernanceDenied, SafetyBlocked};

pub(super) struct UpstreamRelay<'a> {
    pub raw_body: &'a Bytes,
    pub inbound: &'a dyn InboundAdapter,
}

pub(super) struct PreparedDispatch {
    request: CanonicalRequest,
    upstream_model: String,
    model_limits: Option<ModelLimits>,
    body: PreparedBody,
}

pub(super) struct GovernedDispatch(PreparedDispatch);

pub(super) struct ScannedDispatch(PreparedDispatch);

impl PreparedDispatch {
    pub(super) async fn build(
        config: &GatewayConfig,
        upstream: &ResolvedUpstream<'_>,
        mut request: CanonicalRequest,
        audit: &GatewayAudit,
        relay: UpstreamRelay<'_>,
    ) -> Result<Self, DispatchError> {
        let upstream_model = upstream
            .route
            .effective_upstream_model(&request.model)
            .to_owned();
        let override_descriptor = apply_system_prompt_override(
            config,
            &upstream.provider.name,
            &upstream_model,
            &mut request,
        )
        .await;
        if let Some(descriptor) = &override_descriptor {
            audit.set_system_prompt_override(descriptor).await;
        }
        let model_limits = upstream
            .provider
            .find_model(&upstream_model)
            .map(|m| m.limits);
        let raw_body = match &override_descriptor {
            Some(_) => None,
            None => (relay.inbound.passthrough_wire() == Some(upstream.provider.wire))
                .then_some(relay.raw_body),
        };
        strip_caller_identity(&mut request);

        let ctx = outbound_ctx(
            upstream,
            &request,
            CtxParts {
                upstream_model: &upstream_model,
                model_limits,
                forward_headers: &[],
                raw_body,
            },
        );
        let body = upstream
            .adapter
            .build_body(&ctx)
            .map_err(DispatchError::Recorded)?;
        audit.set_prepared_body_digest(&body.bytes).await;

        let surface = inspect::string_leaves(&body.bytes, inspect::SurfaceBudget::default());
        if surface.truncated() {
            tracing::warn!(
                ai_request_id = %audit.ctx.ai_request_id,
                leaves = surface.len(),
                "Gateway inspection surface truncated — part of the forwarded body was not scanned"
            );
        }
        request.forwarded_surface = surface;

        Ok(Self {
            request,
            upstream_model,
            model_limits,
            body,
        })
    }
}

impl GovernedDispatch {
    pub(super) async fn enforce(
        prepared: PreparedDispatch,
        db: &DbPool,
        ctx: &GatewayRequestContext,
        audit: &GatewayAudit,
    ) -> Result<Self, DispatchError> {
        let PromptEvaluation {
            evaluation,
            call_id,
            session_id,
        } = evaluate_prompt(ctx, &prepared.request);

        #[expect(
            clippy::match_same_arms,
            reason = "four distinct governance verdicts that happen to share two bodies; \
                      merging them would delete the reasoning for why each lands where it does"
        )]
        let denied = match &evaluation.decision {
            Decision::Allow { .. } => None,
            // Why: warn mode's entire purpose is that the call proceeds. The
            // reason is already on the audit row written just below, so
            // nothing is lost by not refusing here.
            Decision::Warn { .. } => None,
            Decision::Deny { reason } => Some(reason.to_string()),
            // Why: a held call needs somewhere to park and something to wake
            // it. The MCP enforcement point has both; an inference request on
            // this path has neither, so the only safe reading of "a human must
            // authorise this" here is a refusal. Failing open would turn the
            // strictest verdict in the chain into the weakest.
            Decision::Pending { reason } => Some(reason.to_string()),
        };
        let policy = evaluation
            .chain
            .iter()
            .find(|e| e.result == ChainEntryResult::Fail)
            .map_or_else(
                || "default_allow".to_owned(),
                |e| e.policy_id.as_str().to_owned(),
            );

        record_governance_decision(db, ctx, evaluation, call_id, session_id).await;

        let Some(reason) = denied else {
            return Ok(Self(prepared));
        };
        tracing::warn!(
            ai_request_id = %ctx.ai_request_id,
            user_id = %ctx.user_id,
            policy = %policy,
            reason = %reason,
            "Gateway request denied by governance policy"
        );
        if let Err(e) = audit.fail(&reason).await {
            tracing::warn!(error = %e, "governance-deny audit fail failed");
        }
        Err(DispatchError::Recorded(
            GovernanceDenied {
                policy,
                message: reason,
            }
            .into(),
        ))
    }
}

impl ScannedDispatch {
    pub(super) async fn enforce(
        governed: GovernedDispatch,
        repos: &super::super::GatewayRepositories,
        ai_request_id: &AiRequestId,
        safety: &SafetyConfig,
        audit: &GatewayAudit,
    ) -> Result<Self, DispatchError> {
        let GovernedDispatch(prepared) = governed;
        let findings = run_request_safety_scan(
            &repos.safety_findings,
            ai_request_id,
            &prepared.request,
            safety,
        )
        .await;
        // Why: the same predicate that stamped the `blocked` column decides the
        // refusal, so a finding can never be reported as blocking while the
        // request went through, or the reverse. It is false throughout under
        // `safety.mode: warn`.
        let Some(finding) = findings.iter().find(|f| request_finding_blocks(f, safety)) else {
            return Ok(Self(prepared));
        };
        let msg = format!(
            "request blocked by safety policy: category '{}'",
            finding.category
        );
        tracing::warn!(
            ai_request_id = %ai_request_id,
            category = %finding.category,
            scanner = %finding.scanner,
            "Gateway blocked request by safety policy"
        );
        if let Err(e) = audit.fail(&msg).await {
            tracing::warn!(error = %e, "safety-block audit fail failed");
        }
        Err(DispatchError::Recorded(
            SafetyBlocked {
                category: finding.category.clone(),
                message: msg,
            }
            .into(),
        ))
    }

    pub(super) fn request_model(&self) -> &str {
        &self.0.request.model
    }

    pub(super) async fn send(
        &self,
        upstream: &ResolvedUpstream<'_>,
        forward_headers: &[(String, String)],
        audit: &GatewayAudit,
    ) -> Result<OutboundOutcome, DispatchError> {
        let prepared = &self.0;
        let ctx = outbound_ctx(
            upstream,
            &prepared.request,
            CtxParts {
                upstream_model: &prepared.upstream_model,
                model_limits: prepared.model_limits,
                forward_headers,
                raw_body: None,
            },
        );
        match upstream.adapter.send(ctx, &prepared.body).await {
            Ok(o) => Ok(o),
            Err(e) => {
                audit_upstream_failure(
                    audit,
                    upstream.provider.name.as_str(),
                    &prepared.request.model,
                    &e,
                )
                .await;
                Err(DispatchError::Recorded(e))
            },
        }
    }
}
