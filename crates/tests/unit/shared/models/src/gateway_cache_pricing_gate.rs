//! Boot refuses a catalog model that declares no cache-read rate.
//!
//! Every outbound wire the gateway speaks reports cached prompt tokens, and
//! `CanonicalUsage::input_tokens` is exclusive of them. A model whose pricing
//! omits `cache_read_per_million` therefore bills its cached slice at nothing,
//! and nothing downstream re-checks the rate card. These tests pin the three
//! answers that matter: absent fails and names the model, an explicit `0.0`
//! passes as a deliberate "this provider bills no cache reads", and a real rate
//! passes.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: panics are the assertion mechanism"
)]

use std::collections::HashMap;

use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::services::{
    ApiSurface, GatewayConfig, GatewayProfileError, GatewayRoute, ModelPricing, ProviderEntry,
    ProviderModel, ProviderRegistry, WireProtocol,
};

fn priced(cache_read: Option<f64>) -> ModelPricing {
    ModelPricing {
        input_per_million: 1.25,
        output_per_million: 10.0,
        cache_read_per_million: cache_read,
        cache_write_per_million: None,
        per_image_cents: None,
    }
}

fn registry(cache_read: Option<f64>) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("gemini"),
            wire: WireProtocol::Gemini,
            surface: ApiSurface::Gemini,
            endpoint: "https://example.invalid/v1beta".to_owned(),
            api_key_secret: SecretName::new("gemini"),
            governance: Default::default(),
            extra_headers: HashMap::new(),
            models: vec![ProviderModel {
                id: ModelId::new("gemini-2.5-pro"),
                aliases: Vec::new(),
                governance: None,
                upstream_model: None,
                pricing: priced(cache_read),
                capabilities: Default::default(),
                limits: Default::default(),
            }],
        }],
    }
}

fn config() -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        routes: vec![GatewayRoute {
            id: RouteId::new("gemini-pro"),
            model_pattern: "gemini-2.5-*".to_owned(),
            provider: ProviderId::new("gemini"),
            upstream_model: None,
            extra_headers: HashMap::new(),
            pricing: None,
            when: None,
            requires: None,
        }],
        ..GatewayConfig::default()
    }
}

#[test]
fn a_model_without_a_cache_read_rate_fails_validation() {
    match config().validate(&registry(None)) {
        Err(GatewayProfileError::RouteModelCacheRateUndeclared {
            route,
            provider,
            model,
        }) => {
            assert_eq!(route, "gemini-pro");
            assert_eq!(provider, "gemini");
            assert_eq!(model, "gemini-2.5-pro");
        },
        other => panic!("expected RouteModelCacheRateUndeclared, got {other:?}"),
    }
}

#[test]
fn an_explicit_zero_cache_read_rate_passes() {
    config()
        .validate(&registry(Some(0.0)))
        .expect("an explicit 0.0 is a declared rate, not a forgotten one");
}

#[test]
fn a_real_cache_read_rate_passes() {
    config()
        .validate(&registry(Some(0.31)))
        .expect("a declared rate validates");
}

#[test]
fn an_inline_route_pricing_without_a_cache_rate_also_fails() {
    let mut cfg = config();
    cfg.routes[0].pricing = Some(priced(None));
    match cfg.validate(&registry(Some(0.31))) {
        Err(GatewayProfileError::RouteModelCacheRateUndeclared { model, .. }) => {
            assert_eq!(model, "gemini-2.5-*");
        },
        other => panic!("expected RouteModelCacheRateUndeclared, got {other:?}"),
    }
}
