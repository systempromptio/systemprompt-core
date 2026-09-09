//! Fail-closed admission of text evaluation requests under an attested session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::protocol::canonical::{CanonicalContent, CanonicalRequest};
use super::{GatewayRepositories, GatewayRequestContext};
use anyhow::{Result, ensure};
use systemprompt_evaluation::repository::experiments::{AdmissionRequest, RequestAdmission};
use systemprompt_identifiers::ModelId;
use systemprompt_models::services::ModelPricing;

pub async fn admit(
    repositories: &GatewayRepositories,
    context: &GatewayRequestContext,
    request: &CanonicalRequest,
    encoded_bytes: usize,
    pricing: &ModelPricing,
) -> Result<bool> {
    let Some(session) = context.session_id.as_ref() else {
        return Ok(false);
    };
    if !repositories
        .evaluations
        .is_evaluation_session(session)
        .await?
    {
        return Ok(false);
    }
    ensure!(
        encoded_bytes <= 512_000 && request.max_tokens <= 32768,
        "Evaluation request exceeds frozen text/output envelope"
    );
    ensure!(
        request
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .all(text_only),
        "Evaluation admission currently requires text-only input"
    );
    let bound = request_bound(pricing, encoded_bytes, request.max_tokens)?;
    let model = ModelId::new(request.model.clone());
    let admission = repositories
        .evaluations
        .admit(&AdmissionRequest {
            owner: &context.user_id,
            session,
            request: &context.ai_request_id,
            model: &model,
            bound_microdollars: bound,
        })
        .await?;
    ensure!(
        matches!(admission, RequestAdmission::Reserved(_)),
        "Evaluation binding disappeared before dispatch"
    );
    Ok(true)
}

pub fn request_bound(
    pricing: &ModelPricing,
    encoded_bytes: usize,
    output_tokens: u32,
) -> Result<i64> {
    let input_rate = [
        pricing.input_per_million,
        pricing.cache_read_rate(),
        pricing.cache_write_rate(),
    ]
    .into_iter()
    .try_fold(0.0_f64, |highest, rate| {
        ensure!(rate.is_finite() && rate >= 0.0, "Invalid input pricing");
        Ok::<_, anyhow::Error>(highest.max(rate))
    })?;
    ensure!(
        pricing.output_per_million.is_finite() && pricing.output_per_million >= 0.0,
        "Invalid output pricing"
    );
    ensure!(
        input_rate > 0.0 || pricing.output_per_million > 0.0,
        "Paid evaluations require explicit nonzero pricing"
    );
    let bytes = u32::try_from(encoded_bytes)?;
    let bound = (f64::from(bytes) + 4096.0) * input_rate
        + f64::from(output_tokens) * pricing.output_per_million;
    ensure!(
        bound.is_finite() && bound < i64::MAX as f64,
        "Request bound overflow"
    );
    Ok((bound.ceil() as i64).max(1))
}

fn text_only(content: &CanonicalContent) -> bool {
    match content {
        CanonicalContent::Image(_) => false,
        CanonicalContent::ToolResult { content, .. } => content.iter().all(text_only),
        _ => true,
    }
}

pub async fn preflight(
    repositories: &GatewayRepositories,
    context: &GatewayRequestContext,
    policy: &super::policy::GatewayPolicySpec,
) -> Result<bool> {
    let Some(session) = &context.session_id else {
        return Ok(false);
    };
    if !repositories
        .evaluations
        .is_evaluation_session(session)
        .await?
    {
        return Ok(false);
    }
    ensure!(
        !systemprompt_ai::RouteSelectorEngine::global().has_selectors(),
        "Evaluation requires fixed routing; selector inference is not budget-bound"
    );
    ensure!(
        policy.safety.scanners.is_empty(),
        "Evaluation requires budget-bound safety scanners before this policy can run"
    );
    Ok(true)
}
