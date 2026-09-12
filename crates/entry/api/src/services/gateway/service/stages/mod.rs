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
pub mod recovery;

use bytes::Bytes;
use systemprompt_ai::SafetyConfig;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::services::GatewayConfig;
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_security::authz::types::{Decision, DenyReason};
use systemprompt_security::policy::{ChainEntryResult, SECRET_SCAN_ID};

pub(in crate::services::gateway::service) use self::governance::record_quota_warning;
use self::governance::{PromptEvaluation, evaluate_prompt, record_governance_decision};
use self::outbound::{
    CtxParts, outbound_ctx, resolve_url_images, send_bracketed, strip_caller_identity,
};
use super::super::audit::{GatewayAudit, GatewayRequestContext};
use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::inbound::InboundAdapter;
use super::super::protocol::outbound::{OutboundOutcome, PreparedBody};
use super::finalize::{
    apply_system_prompt_override, request_finding_blocks, run_request_safety_scan,
};
use super::resolve::ResolvedUpstream;
use super::{DispatchError, GovernanceDenied, PromptRepairRequired, SafetyBlocked};

const UNSANITIZABLE_SECRET_MESSAGE: &str = "Secret content could not be safely sanitized; remove \
                                            the affected content or restart with a corrected \
                                            system prompt";
const FALLBACK_REPAIR_LOCATION: &str = "provider_payload";

pub(super) struct UpstreamRelay<'a> {
    pub raw_body: &'a Bytes,
    pub inbound: &'a dyn InboundAdapter,
}

pub(super) struct PreparedDispatch {
    request: CanonicalRequest,
    upstream_model: String,
    model_limits: Option<ModelLimits>,
    body: PreparedBody,
    recovery_count: usize,
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
            .provider
            .upstream_model_for(upstream.route.upstream_model.as_deref(), &request.model)
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
            .find_model(&request.model)
            .map(|m| m.limits);
        let raw_body = match &override_descriptor {
            Some(_) => None,
            None => (relay.inbound.passthrough_wire() == Some(upstream.provider.wire))
                .then_some(relay.raw_body),
        };
        strip_caller_identity(&mut request);
        resolve_url_images(upstream.provider.wire, &mut request, audit).await?;

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

        Ok(Self {
            request,
            upstream_model,
            model_limits,
            body,
            recovery_count: 0,
        })
    }
}

impl GovernedDispatch {
    pub(super) async fn enforce(
        mut prepared: PreparedDispatch,
        db: &DbPool,
        ctx: &GatewayRequestContext,
        audit: &GatewayAudit,
    ) -> Result<Self, DispatchError> {
        let PromptEvaluation {
            evaluation,
            call_id,
            session_id,
            recovery_count,
            recovery_locations,
        } = evaluate_prompt(ctx, &mut prepared.request, &mut prepared.body)
            .map_err(|error| DispatchError::PreAudit(error.into()))?;

        prepared.recovery_count = recovery_count;
        if recovery_count > 0 {
            audit.set_prepared_body_digest(&prepared.body.bytes).await;
            tracing::warn!(
                ai_request_id = %ctx.ai_request_id,
                recovery_count,
                locations = ?recovery_locations,
                "Gateway sanitized secret-bearing prompt content"
            );
        }

        #[expect(
            clippy::match_same_arms,
            reason = "four distinct governance verdicts that happen to share two bodies; \
                      merging them would delete the reasoning for why each lands where it does"
        )]
        let denied = match &evaluation.decision {
            Decision::Allow { .. } => None,
            Decision::Warn { .. } => None,
            Decision::Deny {
                reason: DenyReason::SecretLeak { .. },
            } => Some(UNSANITIZABLE_SECRET_MESSAGE.to_owned()),
            Decision::Deny { reason } => Some(reason.to_string()),
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

        record_governance_decision(db, ctx, evaluation, call_id, session_id)
            .await.map_err(super::DispatchError::Recorded)?;

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
        Err(governance_denial(policy, reason, recovery_locations))
    }
}

fn governance_denial(policy: String, message: String, mut locations: Vec<String>) -> DispatchError {
    if policy != SECRET_SCAN_ID {
        return DispatchError::Recorded(GovernanceDenied { policy, message }.into());
    }
    if locations.is_empty() {
        locations.push(FALLBACK_REPAIR_LOCATION.to_owned());
    }
    DispatchError::Recorded(PromptRepairRequired { message, locations }.into())
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

    pub(super) async fn admit_evaluation(
        &self,
        repositories: &crate::services::gateway::GatewayRepositories,
        context: &GatewayRequestContext,
        pricing: &systemprompt_models::services::ModelPricing,
    ) -> Result<bool, DispatchError> {
        crate::services::gateway::evaluation::admit(
            repositories,
            context,
            &self.0.request,
            self.0.body.bytes.len(),
            pricing,
        )
        .await
        .map_err(DispatchError::Recorded)
    }

    pub(super) const fn recovery_count(&self) -> usize {
        self.0.recovery_count
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
        send_bracketed(
            upstream,
            ctx,
            &prepared.body,
            &prepared.request.model,
            audit,
        )
        .await
    }
}
