//! Unit tests for the advertised model catalog behind `/v1/bridge/profile` and
//! `/v1/models`. The gateway transcodes every inbound wire to every provider
//! wire, so both front doors advertise the *whole* advertised set: the flat
//! `models` list is never narrowed by surface, and `model_entries` is called
//! with an empty surface slice. Only a `surface: backend` provider is withheld.
//! The per-surface views that remain (Claude Desktop's carve-out) are built by
//! the bridge from `providers`, and `surfaces_from_header` still validates the
//! advisory `x-inference-protocol` header.

use std::collections::HashMap;
use systemprompt_manifest::services::providers::surface_for;

use axum::http::HeaderMap;
use systemprompt_api::error::ApiHttpError;
use systemprompt_api::routes::gateway::bridge::provider_health;
use systemprompt_api::routes::gateway::models::{model_entries, surfaces_from_header};
use systemprompt_identifiers::headers::INFERENCE_PROTOCOL;
use systemprompt_identifiers::{ModelId, ProviderId, SecretName, TenantId};
use systemprompt_manifest::bridge_profile::{BridgeProfileParams, build as profile_build};
use systemprompt_manifest::services::{
    GatewayConfig, GatewayRoute, ProviderEntry, ProviderModel, ProviderRegistry,
};
use systemprompt_models::api::ErrorCode;
use systemprompt_models::bridge::profile::BridgeProfileResponse;
use systemprompt_models::providers::ApiSurface;
use systemprompt_wire::WireProtocol;

fn build_profile(registry: &ProviderRegistry) -> BridgeProfileResponse {
    profile_build(
        BridgeProfileParams {
            inference_gateway_base_url: "https://gw.invalid/v1".to_owned(),
            auth_scheme: "bearer".to_owned(),
            organization_uuid: None,
            default_model: None,
            registry,
            gateway: None,
        },
        |_| true,
    )
}

fn build_models(registry: &ProviderRegistry) -> Vec<String> {
    build_profile(registry).models
}

fn model(id: &str, aliases: &[&str]) -> ProviderModel {
    ProviderModel {
        id: ModelId::new(id),
        aliases: aliases.iter().map(|a| ModelId::new(*a)).collect(),
        hidden: false,
        governance: None,
        upstream_model: None,
        pricing: Default::default(),
        capabilities: Default::default(),
        limits: Default::default(),
    }
}

fn provider(name: &str, wire: WireProtocol, models: Vec<ProviderModel>) -> ProviderEntry {
    provider_with_secret(name, wire, name, models)
}

fn provider_with_secret(
    name: &str,
    wire: WireProtocol,
    secret: &str,
    models: Vec<ProviderModel>,
) -> ProviderEntry {
    provider_with_surface(name, wire, surface_for(wire), secret, models)
}

fn provider_with_surface(
    name: &str,
    wire: WireProtocol,
    surface: ApiSurface,
    secret: &str,
    models: Vec<ProviderModel>,
) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new(name),
        display_name: None,
        description: None,
        wire,
        surface,
        endpoint: "https://example.invalid/v1".to_owned(),
        api_key_secret: Some(SecretName::new(secret)),
        credential: Default::default(),
        credential_from: None,
        user_agent: None,
        forward_headers: Vec::new(),
        governance: Default::default(),
        extra_headers: Default::default(),
        accepted_betas: None,
        models,
    }
}

#[test]
fn advertises_canonical_ids_without_aliases() {
    let registry = ProviderRegistry {
        providers: vec![provider(
            "anthropic",
            WireProtocol::Anthropic,
            vec![
                model("claude-sonnet-4-6", &["claude-sonnet"]),
                model("claude-haiku-4-5", &[]),
            ],
        )],
    };

    let models = registry.advertised_model_ids(&[ApiSurface::Anthropic]);

    assert_eq!(
        models,
        vec![
            "claude-sonnet-4-6".to_owned(),
            "claude-haiku-4-5".to_owned(),
        ]
    );
    assert!(registry.contains_model("claude-sonnet"));
}

#[test]
fn includes_every_advertised_surface() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &["claude-sonnet"])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &["gemini-flash"])],
            ),
            provider(
                "openai",
                WireProtocol::OpenAiChat,
                vec![model("gpt-5", &[])],
            ),
        ],
    };

    let models = build_models(&registry);

    for expected in [
        "claude-sonnet-4-6",
        "gemini-3.1-flash-lite-preview",
        "gpt-5",
    ] {
        assert!(
            models.iter().any(|m| m == expected),
            "{expected} missing from {models:?}"
        );
    }
    assert!(!models.iter().any(|model| model == "claude-sonnet"));
    assert!(!models.iter().any(|model| model == "gemini-flash"));
}

#[test]
fn empty_only_when_every_provider_is_backend() {
    let gemini_only = ProviderRegistry {
        providers: vec![provider(
            "gemini",
            WireProtocol::Gemini,
            vec![model("gemini-3.1-flash-lite-preview", &[])],
        )],
    };
    assert_eq!(
        build_models(&gemini_only),
        vec!["gemini-3.1-flash-lite-preview".to_owned()],
        "a non-Anthropic provider is still advertised"
    );

    let backend_only = ProviderRegistry {
        providers: vec![provider_with_surface(
            "minimax",
            WireProtocol::Anthropic,
            ApiSurface::Backend,
            "minimax_key",
            vec![model("MiniMax-M2", &[])],
        )],
    };
    assert!(build_models(&backend_only).is_empty());
}

#[test]
fn backend_provider_is_absent_from_models_and_providers() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider_with_surface(
                "minimax",
                WireProtocol::Anthropic,
                ApiSurface::Backend,
                "minimax_key",
                vec![model("MiniMax-M2", &[])],
            ),
        ],
    };

    let response = build_profile(&registry);

    assert_eq!(response.models, vec!["claude-sonnet-4-6".to_owned()]);
    assert!(response.providers.iter().all(|p| p.name != "minimax"));
}

#[test]
fn empty_protocols_returns_full_catalog() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &[])],
            ),
        ],
    };

    let models = registry.advertised_model_ids(&[]);

    assert!(models.iter().any(|m| m == "claude-sonnet-4-6"));
    assert!(models.iter().any(|m| m == "gemini-3.1-flash-lite-preview"));
}

#[test]
fn model_entries_returns_the_whole_catalog() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &["gemini-flash"])],
            ),
        ],
    };

    let entries = model_entries(&registry, &[], None, |_| true);

    let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["claude-sonnet-4-6", "gemini-3.1-flash-lite-preview"]
    );
    assert!(entries.iter().all(|e| e.kind == "model"));
}

#[test]
fn model_entries_still_honours_an_explicit_surface_slice() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &[])],
            ),
        ],
    };

    let ids: Vec<String> = model_entries(&registry, &[ApiSurface::Anthropic], None, |_| true)
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, vec!["claude-sonnet-4-6".to_owned()]);
}

#[test]
fn header_helper_parses_known_tags_and_defaults_to_empty() {
    let mut headers = HeaderMap::new();
    assert_eq!(surfaces_from_header(&headers).unwrap(), Vec::new());

    headers.insert(INFERENCE_PROTOCOL, "anthropic, gemini".parse().unwrap());
    assert_eq!(
        surfaces_from_header(&headers).unwrap(),
        vec![ApiSurface::Anthropic, ApiSurface::Gemini]
    );
}

#[test]
fn header_helper_rejects_backend_and_garbage() {
    for bad in ["backend", "not-a-protocol"] {
        let mut headers = HeaderMap::new();
        headers.insert(INFERENCE_PROTOCOL, bad.parse().unwrap());
        let err = ApiHttpError::from(surfaces_from_header(&headers).unwrap_err()).into_inner();
        assert_eq!(err.code, ErrorCode::BadRequest, "{bad}");
    }
}

#[test]
fn provider_health_reports_configured_and_models() {
    let registry = ProviderRegistry {
        providers: vec![provider_with_secret(
            "anthropic",
            WireProtocol::Anthropic,
            "anthropic_key",
            vec![model("claude-sonnet-4-6", &["claude-sonnet"])],
        )],
    };

    let health = provider_health(&registry, |name| name == "anthropic_key");

    assert_eq!(health.len(), 1);
    let entry = &health[0];
    assert_eq!(entry.name, "anthropic");
    assert_eq!(entry.surface, ApiSurface::Anthropic);
    assert!(entry.configured);
    assert!(entry.config_issue.is_none());
    assert_eq!(entry.models, vec!["claude-sonnet-4-6".to_owned()]);
}

#[test]
fn hidden_models_remain_routable_but_are_not_advertised() {
    let mut legacy = model("claude-opus-4-8", &["claude-opus-legacy"]);
    legacy.hidden = true;
    let registry = ProviderRegistry {
        providers: vec![provider(
            "anthropic",
            WireProtocol::Anthropic,
            vec![model("claude-opus-5", &[]), legacy],
        )],
    };

    let profile = build_profile(&registry);

    assert_eq!(profile.models, vec!["claude-opus-5".to_owned()]);
    assert_eq!(profile.providers[0].models, profile.models);
    assert!(registry.contains_model("claude-opus-4-8"));
    assert!(registry.contains_model("claude-opus-legacy"));
}

#[test]
fn provider_health_flags_missing_secret() {
    let registry = ProviderRegistry {
        providers: vec![
            provider_with_secret(
                "anthropic",
                WireProtocol::Anthropic,
                "anthropic_key",
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider_with_secret(
                "gemini",
                WireProtocol::Gemini,
                "gemini_key",
                vec![model("gemini-3.1-flash-lite-preview", &[])],
            ),
        ],
    };

    let health = provider_health(&registry, |name| name == "anthropic_key");

    let gemini = health.iter().find(|h| h.name == "gemini").unwrap();
    assert!(!gemini.configured);
    assert_eq!(
        gemini.config_issue.as_deref(),
        Some("API key secret 'gemini_key' is not configured")
    );

    let anthropic = health.iter().find(|h| h.name == "anthropic").unwrap();
    assert!(anthropic.configured);
}

#[test]
fn provider_health_excludes_backend_provider() {
    let registry = ProviderRegistry {
        providers: vec![
            provider_with_secret(
                "anthropic",
                WireProtocol::Anthropic,
                "anthropic_key",
                vec![model("claude-3-7-sonnet-20250219", &[])],
            ),
            provider_with_surface(
                "minimax",
                WireProtocol::Anthropic,
                ApiSurface::Backend,
                "minimax_key",
                vec![model("MiniMax-M2", &[])],
            ),
        ],
    };

    let health = provider_health(&registry, |_| true);

    assert!(
        health.iter().all(|h| h.name != "minimax"),
        "backend provider must never appear in bridge profile provider health"
    );
    assert!(
        !health
            .iter()
            .flat_map(|h| h.models.iter())
            .any(|m| m == "MiniMax-M2")
    );
}

#[test]
fn canonicalize_org_uuid_peels_local_prefix_from_uuid_tenant() {
    let tenant = TenantId::new("local_6fa459ea-ee8a-3ca4-894e-db77e160355e");
    assert_eq!(
        tenant.organization_uuid(),
        "6fa459ea-ee8a-3ca4-894e-db77e160355e"
    );
}

#[test]
fn canonicalize_org_uuid_normalizes_bare_uuid_to_lowercase_canonical() {
    let tenant = TenantId::new("6FA459EA-EE8A-3CA4-894E-DB77E160355E");
    assert_eq!(
        tenant.organization_uuid(),
        "6fa459ea-ee8a-3ca4-894e-db77e160355e"
    );
}

#[test]
fn canonicalize_org_uuid_derives_stable_v5_for_non_uuid_tenant() {
    let tenant = TenantId::new("local_acme-corp");
    let got = tenant.organization_uuid();
    assert_eq!(got.len(), 36, "{got}");
    assert_eq!(got.as_bytes()[14], b'5', "expected a v5 UUID: {got}");
    assert_eq!(
        got,
        tenant.organization_uuid(),
        "derivation must be deterministic"
    );
    assert_ne!(
        got,
        TenantId::new("local_other-corp").organization_uuid(),
        "distinct tenants must not collide"
    );
}

fn two_provider_registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![
            provider_with_secret(
                "anthropic",
                WireProtocol::Anthropic,
                "anthropic_key",
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider_with_secret(
                "vertex-maas",
                WireProtocol::OpenAiChat,
                "vertex_maas",
                vec![model("qwen3-coder", &[])],
            ),
        ],
    }
}

fn routed(pattern: &str, provider: &str) -> GatewayConfig {
    let mut route = GatewayRoute {
        id: None,
        name: None,
        description: None,
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new(provider),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
        fallbacks: Vec::new(),
        by_scope: None,
        strategy: systemprompt_manifest::services::SelectionStrategy::Ordered,
        weight: None,
        context_fallbacks: Vec::new(),
    };
    route.ensure_id();
    GatewayConfig {
        enabled: true,
        routes: vec![route],
        ..GatewayConfig::default()
    }
}

#[test]
fn models_of_an_unconfigured_provider_are_not_advertised() {
    let registry = two_provider_registry();

    let ids: Vec<String> = model_entries(&registry, &[], None, |name| name == "anthropic_key")
        .into_iter()
        .map(|e| e.id)
        .collect();

    assert_eq!(ids, vec!["claude-sonnet-4-6".to_owned()]);
}

#[test]
fn the_profile_drops_unservable_models_but_still_reports_their_provider() {
    let registry = two_provider_registry();

    let response = profile_build(
        BridgeProfileParams {
            inference_gateway_base_url: "https://gw.invalid/v1".to_owned(),
            auth_scheme: "bearer".to_owned(),
            organization_uuid: None,
            default_model: None,
            registry: &registry,
            gateway: None,
        },
        |name| name == "anthropic_key",
    );

    assert_eq!(response.models, vec!["claude-sonnet-4-6".to_owned()]);
    assert!(!response.model_limits.contains_key("qwen3-coder"));
    let maas = response
        .providers
        .iter()
        .find(|p| p.name == "vertex-maas")
        .expect("an unconfigured provider stays visible to admins");
    assert!(!maas.configured);
}

#[test]
fn a_model_routed_to_a_configured_provider_is_advertised() {
    let registry = two_provider_registry();
    let gateway = routed("qwen3-*", "anthropic");

    let ids: Vec<String> = model_entries(&registry, &[], Some(&gateway), |name| {
        name == "anthropic_key"
    })
    .into_iter()
    .map(|e| e.id)
    .collect();

    assert_eq!(
        ids,
        vec!["claude-sonnet-4-6".to_owned(), "qwen3-coder".to_owned()]
    );
}

#[test]
fn a_model_routed_to_an_unconfigured_provider_is_withheld() {
    let registry = two_provider_registry();
    let gateway = routed("claude-*", "vertex-maas");

    let ids: Vec<String> = model_entries(&registry, &[], Some(&gateway), |name| {
        name == "anthropic_key"
    })
    .into_iter()
    .map(|e| e.id)
    .collect();

    assert!(ids.is_empty(), "{ids:?}");
}

mod admission {
    //! `ModelAdmission` against dispatch: for every model the profile could
    //! list, admission says yes exactly when `/v1/messages` would route,
    //! serve and entitle it — and asking never writes an audit row.

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use axum::http::{HeaderMap, HeaderValue};
    use systemprompt_api::error::ApiHttpError;
    use systemprompt_api::routes::gateway::bridge::profile;
    use systemprompt_api::routes::gateway::bridge_admission::{
        AdmissionPrincipal, ModelAdmission, probe_route,
    };
    use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
    use systemprompt_governance::authz::{
        AuthzDecision, AuthzDecisionHook, AuthzRequest, DenyReason, EntityRef, SharedAuthzHook,
    };
    use systemprompt_identifiers::{RouteId, SessionId, UserId};
    use systemprompt_manifest::services::{GatewayConfig, ProviderRegistry};
    use systemprompt_models::api::ErrorCode;
    use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context, test_db_pool};
    use systemprompt_traits::AppContext as _;
    use systemprompt_wire::WireProtocol;

    use super::{model, provider_with_secret, routed};

    #[derive(Debug, Default)]
    struct DenyRoute {
        denied: Option<RouteId>,
        evaluated: AtomicUsize,
    }

    impl DenyRoute {
        fn decide(&self, req: &AuthzRequest) -> AuthzDecision {
            match (&req.entity, &self.denied) {
                (EntityRef::GatewayRoute(route), Some(denied)) if route == denied => {
                    AuthzDecision::Deny {
                        reason: DenyReason::PolicyViolation {
                            policy: "test".to_owned(),
                            detail: std::borrow::Cow::Borrowed("not entitled"),
                        },
                        policy: "test".to_owned(),
                    }
                },
                _ => AuthzDecision::Allow,
            }
        }
    }

    #[async_trait]
    impl AuthzDecisionHook for DenyRoute {
        async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
            self.evaluated.fetch_add(1, Ordering::SeqCst);
            self.decide(&req)
        }

        async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
            self.decide(&req)
        }
    }

    fn registry() -> ProviderRegistry {
        let mut legacy = model("claude-legacy", &[]);
        legacy.hidden = true;
        let mut secret = model("claude-secret", &[]);
        secret.hidden = true;
        ProviderRegistry {
            providers: vec![
                provider_with_secret(
                    "anthropic",
                    WireProtocol::Anthropic,
                    "anthropic_key",
                    vec![model("claude-sonnet-4-6", &[]), legacy, secret],
                ),
                provider_with_secret(
                    "vertex-maas",
                    WireProtocol::OpenAiChat,
                    "vertex_maas",
                    vec![model("qwen3-coder", &[])],
                ),
            ],
        }
    }

    fn gateway() -> GatewayConfig {
        let mut gateway = routed("claude-legacy", "anthropic");
        gateway
            .routes
            .extend(routed("claude-sonnet-*", "anthropic").routes);
        gateway
            .routes
            .extend(routed("qwen3-*", "vertex-maas").routes);
        gateway
    }

    fn principal() -> AdmissionPrincipal {
        AdmissionPrincipal {
            user_id: UserId::new("admission-user"),
            roles: vec!["user".to_owned()],
            client_id: None,
            session_id: SessionId::generate(),
        }
    }

    fn servable(model: &str) -> bool {
        systemprompt_manifest::bridge_profile::is_model_servable(
            &registry(),
            Some(&gateway()),
            model,
            &|name: &str| name == "anthropic_key",
        )
    }

    async fn dispatch_would_serve(
        gateway: &GatewayConfig,
        registry: &ProviderRegistry,
        hook: &SharedAuthzHook,
        model: &str,
    ) -> bool {
        let request = systemprompt_wire::canonical::CanonicalRequest::new(
            systemprompt_identifiers::ModelId::new(model),
            Vec::new(),
            1,
        );
        let Some(route) = gateway.resolve_route(registry, &request) else {
            return false;
        };
        if !servable(model) {
            return false;
        }
        let p = principal();
        let req = systemprompt_api::routes::gateway::messages::build_gateway_authz_request(
            systemprompt_api::routes::gateway::messages::GatewayAuthzRequestInput {
                user_id: p.user_id,
                roles: p.roles,
                attributes: std::collections::BTreeMap::new(),
                act_chain: Vec::new(),
                trace_id: systemprompt_identifiers::TraceId::generate(),
                route_id: route.effective_id(),
                model: systemprompt_identifiers::ModelId::new(model),
                session_id: Some(p.session_id.clone()),
                client_id: None,
                context_id: systemprompt_identifiers::ContextId::derived_from_session(
                    &p.session_id,
                ),
            },
        );
        matches!(hook.evaluate(req).await, AuthzDecision::Allow)
    }

    #[tokio::test]
    async fn admission_equals_dispatch_for_hidden_unentitled_and_unconfigured_models() {
        let registry = registry();
        let gateway = gateway();
        let sonnet_route = probe_route(&gateway, &registry, "claude-sonnet-4-6")
            .expect("sonnet routes")
            .effective_id();
        let deny = Arc::new(DenyRoute {
            denied: Some(sonnet_route),
            ..DenyRoute::default()
        });
        let hook: SharedAuthzHook = deny.clone();
        let p = principal();
        let admission = ModelAdmission {
            gateway: &gateway,
            registry: &registry,
            hook: &hook,
            principal: &p,
            servable: &servable,
        };

        for (model, expected) in [
            ("claude-legacy", true),
            ("claude-secret", false),
            ("claude-sonnet-4-6", false),
            ("qwen3-coder", false),
        ] {
            let admitted = admission.admits(model).await;
            assert_eq!(admitted, expected, "{model}");
            assert_eq!(
                admitted,
                dispatch_would_serve(&gateway, &registry, &hook, model).await,
                "admission and dispatch disagree on {model}"
            );
        }
        let evaluated_by_dispatch = deny.evaluated.load(Ordering::SeqCst);
        let _ = admission.retain(vec!["claude-legacy".to_owned()]).await;
        assert_eq!(
            deny.evaluated.load(Ordering::SeqCst),
            evaluated_by_dispatch,
            "admission probes; it never evaluates, so it writes no audit row"
        );
    }

    #[tokio::test]
    async fn a_profile_request_with_a_bad_bearer_is_401_never_anonymous() {
        let boot = ensure_test_bootstrap();
        let pool = test_db_pool().await;
        let ctx = test_app_context(&pool, &boot.database_url);
        let extractor = Arc::new(JwtContextExtractor::new(
            ctx.session_provider().expect("session provider"),
            ctx.user_provider().expect("user provider"),
            JtiRevocationChecker::from_repository(
                ctx.oauth_repositories().oauth.clone(),
                systemprompt_traits::CacheEpochs::new(),
            ),
            ctx.config().jwt_issuer.clone(),
        ));
        for bearer in [Some("Bearer not-a-token"), None] {
            let mut headers = HeaderMap::new();
            if let Some(value) = bearer {
                headers.insert("authorization", HeaderValue::from_static(value));
            }
            let err = profile(Arc::clone(&extractor), (*ctx).clone(), headers)
                .await
                .expect_err("no verified caller, no profile");
            let err: ApiHttpError = err;
            assert_eq!(err.into_inner().code, ErrorCode::Unauthorized, "{bearer:?}");
        }
    }
}
