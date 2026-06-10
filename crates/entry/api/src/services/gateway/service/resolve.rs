//! Pre-dispatch upstream resolution: model-exposure check, route and provider
//! lookup, API-key secret, and outbound wire adapter.

use std::borrow::Cow;
use std::sync::Arc;

use anyhow::anyhow;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::profile::{GatewayConfig, GatewayRoute, ProviderEntry, ProviderRegistry};

use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::outbound::OutboundAdapter;
use super::super::registry::GatewayUpstreamRegistry;
use super::{DispatchError, PolicyDenied};

pub(super) struct ResolvedUpstream<'a> {
    pub(super) route: Cow<'a, GatewayRoute>,
    pub(super) provider: &'a ProviderEntry,
    pub(super) api_key: &'static str,
    pub(super) adapter: &'static Arc<dyn OutboundAdapter>,
}

pub(super) fn resolve_upstream<'a>(
    config: &'a GatewayConfig,
    registry: &'a ProviderRegistry,
    request: &CanonicalRequest,
    ai_request_id: &AiRequestId,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    if !config.is_model_exposed(registry, &request.model) {
        tracing::warn!(
            ai_request_id = %ai_request_id,
            model = %request.model,
            "Gateway denied: model not exposed by gateway policy or registry"
        );
        return Err(DispatchError::PreAudit(
            PolicyDenied(format!(
                "model '{}' is not permitted by gateway policy",
                request.model
            ))
            .into(),
        ));
    }

    let route = config
        .resolve_route(registry, &request.model)
        .ok_or_else(|| {
            DispatchError::PreAudit(anyhow!(
                "No gateway route matches model '{}'",
                request.model
            ))
        })?;

    let provider = route.resolve(registry).ok_or_else(|| {
        DispatchError::PreAudit(anyhow!(
            "Gateway route '{}' provider '{}' is not declared in profile.providers",
            route.id.as_str(),
            route.provider.as_str()
        ))
    })?;

    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|e| DispatchError::PreAudit(anyhow!("Secrets not available: {e}")))?;

    let api_key = secrets
        .get(provider.api_key_secret.as_str())
        .ok_or_else(|| {
            DispatchError::PreAudit(anyhow!(
                "Gateway API key secret '{}' not configured",
                provider.api_key_secret.as_str()
            ))
        })?;

    let adapter = GatewayUpstreamRegistry::global()
        .get(provider.wire.as_tag())
        .ok_or_else(|| {
            DispatchError::PreAudit(anyhow!(
                "Gateway has no outbound adapter for wire protocol '{}'",
                provider.wire.as_tag()
            ))
        })?;

    Ok(ResolvedUpstream {
        route,
        provider,
        api_key,
        adapter,
    })
}
