use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use std::time::Instant;
use systemprompt_models::profile::GatewayConfig;

use super::models::AnthropicGatewayRequest;
use super::registry::GatewayUpstreamRegistry;
use super::upstream::UpstreamCtx;

#[derive(Debug, Clone, Copy)]
pub struct GatewayService;

impl GatewayService {
    pub async fn dispatch(
        config: &GatewayConfig,
        request: AnthropicGatewayRequest,
        raw_body: Bytes,
    ) -> Result<Response<Body>> {
        let route = config
            .find_route(&request.model)
            .ok_or_else(|| anyhow!("No gateway route matches model '{}'", request.model))?;

        let secrets = systemprompt_models::SecretsBootstrap::get()
            .map_err(|e| anyhow!("Secrets not available: {e}"))?;

        let upstream_api_key = secrets.get(&route.api_key_secret).ok_or_else(|| {
            anyhow!(
                "Gateway API key secret '{}' not configured",
                route.api_key_secret
            )
        })?;

        let upstream = GatewayUpstreamRegistry::global()
            .get(&route.provider)
            .ok_or_else(|| anyhow!("Gateway provider '{}' is not registered", route.provider))?;

        let is_streaming = request.stream.unwrap_or(false);
        let start = Instant::now();

        tracing::info!(
            model = %request.model,
            provider = %route.provider,
            upstream = %route.endpoint,
            streaming = is_streaming,
            "Gateway request dispatched"
        );

        let ctx = UpstreamCtx {
            route,
            api_key: upstream_api_key,
            raw_body,
            request: &request,
            is_streaming,
        };

        let response = upstream.proxy(ctx).await?;

        let latency = start.elapsed().as_millis();
        tracing::info!(
            latency_ms = latency,
            model = %request.model,
            "Gateway request completed"
        );

        Ok(response)
    }
}
