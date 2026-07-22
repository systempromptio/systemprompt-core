use std::borrow::Cow;
use std::collections::HashMap;

use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::profile::{
    ApiSurface, GatewayConfig, GatewayConfigSpec, GatewayProfileError, GatewayRoute, GatewayState,
    OverrideRuleAction, ProviderEntry, ProviderModel, ProviderRegistry, ResponseFormatKind,
    RouteMatch, SystemPromptRule, WireProtocol, default_resource_audiences, slugify_pattern,
    synthesize_route_id,
};
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, ReasoningEffort,
    ResponseFormat, Role, ThinkingConfig,
};

fn req(model: &str) -> CanonicalRequest {
    CanonicalRequest {
        model: model.to_owned(),
        system: None,
        messages: Vec::new(),
        max_tokens: 0,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
}

fn route(pattern: &str) -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new(""),
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new("test"),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
    }
}

#[test]
fn exact_pattern_matches() {
    assert!(route("claude-sonnet-4-6").matches("claude-sonnet-4-6"));
    assert!(!route("claude-sonnet-4-6").matches("claude-opus-4-7"));
}

#[test]
fn prefix_wildcard_matches() {
    assert!(route("claude-*").matches("claude-sonnet-4-6"));
    assert!(!route("claude-*").matches("moonshot-v1-8k"));
}

#[test]
fn catch_all_matches() {
    assert!(route("*").matches("any-model-name"));
}

#[test]
fn route_finds_matching_model() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![GatewayRoute {
            id: RouteId::new(""),
            model_pattern: "kimi-*".to_owned(),
            provider: ProviderId::new("moonshot"),
            upstream_model: Some("moonshot-v1-32k".to_owned()),
            extra_headers: HashMap::new(),
            pricing: None,
            when: None,
        }],
        ..GatewayConfig::default()
    };
    let matched = config.find_route("kimi-latest").expect("route must match");
    assert_eq!(matched.provider.as_str(), "moonshot");
    assert_eq!(
        matched.effective_upstream_model("kimi-latest"),
        "moonshot-v1-32k"
    );
}

#[test]
fn slugify_replaces_star_and_non_alnum() {
    assert_eq!(slugify_pattern("claude-*"), "claude-star");
    assert_eq!(slugify_pattern("foo/bar baz!"), "foo-bar-baz");
    assert_eq!(slugify_pattern("---"), "route");
    assert_eq!(slugify_pattern(""), "route");
    assert_eq!(slugify_pattern("Claude_3.7"), "claude-3-7");
}

#[test]
fn synthesize_route_id_is_stable_and_input_dependent() {
    let a = synthesize_route_id("claude-*", "anthropic");
    let b = synthesize_route_id("claude-*", "anthropic");
    assert_eq!(a, b, "synthesize_route_id must be deterministic");
    assert!(a.as_str().starts_with("claude-star-"));

    let c = synthesize_route_id("claude-*", "openai");
    assert_ne!(a, c, "provider change must produce a different id");

    let d = synthesize_route_id("gpt-*", "anthropic");
    assert_ne!(a, d, "model_pattern change must produce a different id");
}

#[test]
fn synthesize_route_id_matches_golden_fnv1a_digests() {
    let cases = [
        ("*", "minimax", "star-2a5453"),
        ("*", "gemini", "star-aac356"),
        ("claude-*", "anthropic", "claude-star-4203d1"),
        ("claude-*", "openai", "claude-star-4f8d12"),
        ("gpt-*", "anthropic", "gpt-star-f15ce8"),
        ("claude-opus-4-8", "gemini", "claude-opus-4-8-46a2bc"),
    ];
    for (pattern, provider, expected) in cases {
        assert_eq!(
            synthesize_route_id(pattern, provider).as_str(),
            expected,
            "FNV-1a route id drifted for ({pattern}, {provider}): a hash-algorithm \
             change has re-keyed gateway routes; fix the regression, do not rebaseline"
        );
    }
}

#[test]
fn ensure_id_backfills_empty_id() {
    let mut r = route("claude-*");
    assert!(r.id.as_str().is_empty());
    r.ensure_id();
    assert_eq!(r.id, synthesize_route_id("claude-*", "test"));
    let preserved = r.id.clone();
    r.ensure_id();
    assert_eq!(r.id, preserved, "ensure_id must be idempotent");
}

// SSRF endpoint validation now lives on the ProviderRegistry: the gateway owns
// no catalog, and the registry is the authority for outbound connectivity.
fn registry_with_endpoint(endpoint: &str) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("test"),
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            endpoint: endpoint.to_owned(),
            api_key_secret: SecretName::new("test"),
            extra_headers: HashMap::new(),
            models: vec![ProviderModel {
                id: ModelId::new("any"),
                aliases: Vec::new(),
                upstream_model: None,
                pricing: Default::default(),
                capabilities: Default::default(),
                limits: Default::default(),
            }],
        }],
    }
}

#[test]
fn registry_validate_accepts_public_https_endpoint() {
    assert!(
        registry_with_endpoint("https://api.anthropic.com/v1")
            .validate()
            .is_ok()
    );
}

#[test]
fn registry_validate_allows_loopback_http_for_local_dev() {
    assert!(
        registry_with_endpoint("http://localhost:8080")
            .validate()
            .is_ok()
    );
    assert!(
        registry_with_endpoint("http://127.0.0.1:8080")
            .validate()
            .is_ok()
    );
}

#[test]
fn registry_validate_rejects_cloud_metadata_endpoint() {
    assert!(
        registry_with_endpoint("http://169.254.169.254/latest/meta-data/")
            .validate()
            .is_err()
    );
}

#[test]
fn registry_validate_rejects_private_ranges() {
    for endpoint in [
        "https://10.0.0.5/v1",
        "https://192.168.1.10/v1",
        "https://172.16.0.1/v1",
        "https://[fd00::1]/v1",
    ] {
        assert!(
            registry_with_endpoint(endpoint).validate().is_err(),
            "expected {endpoint} to be rejected as a private/ULA address"
        );
    }
}

#[test]
fn registry_validate_rejects_non_http_scheme_and_plain_http_to_remote() {
    assert!(
        registry_with_endpoint("ftp://example.com/v1")
            .validate()
            .is_err()
    );
    assert!(
        registry_with_endpoint("http://api.anthropic.com/v1")
            .validate()
            .is_err()
    );
}

#[test]
fn validate_rejects_duplicate_route_id() {
    let mut a = route("claude-*");
    a.id = RouteId::new("dup");
    let mut b = route("gpt-*");
    b.id = RouteId::new("dup");
    let config = GatewayConfig {
        enabled: true,
        routes: vec![a, b],
        ..GatewayConfig::default()
    };
    let registry = two_provider_registry();
    match config.validate(&registry) {
        Err(GatewayProfileError::DuplicateRouteId { id }) => assert_eq!(id, "dup"),
        other => panic!("expected DuplicateRouteId, got {other:?}"),
    }
}

fn provider_entry(name: &str, endpoint: &str, models: Vec<ProviderModel>) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new(name),
        wire: WireProtocol::Anthropic,
        surface: ApiSurface::Anthropic,
        endpoint: endpoint.to_owned(),
        api_key_secret: SecretName::new(name),
        extra_headers: HashMap::new(),
        models,
    }
}

fn model(id: &str) -> ProviderModel {
    ProviderModel {
        id: ModelId::new(id),
        aliases: Vec::new(),
        upstream_model: None,
        pricing: Default::default(),
        capabilities: Default::default(),
        limits: Default::default(),
    }
}

fn two_provider_registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![
            provider_entry(
                "anthropic",
                "https://api.anthropic.com/v1",
                vec![model("claude-sonnet-4-20250514")],
            ),
            provider_entry(
                "gemini",
                "https://generativelanguage.googleapis.com/v1beta",
                vec![model("gemini-2.5-flash")],
            ),
        ],
    }
}

fn two_provider_config(default_provider: Option<&str>) -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        routes: vec![
            route_to("claude-*", "anthropic"),
            route_to("gemini-*", "gemini"),
        ],
        default_provider: default_provider.map(ProviderId::new),
        ..GatewayConfig::default()
    }
}

fn route_to(pattern: &str, provider: &str) -> GatewayRoute {
    let mut r = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new(provider),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
    };
    r.ensure_id();
    r
}

#[test]
fn resolve_route_prefers_explicit_match_over_default() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let resolved = config
        .resolve_route(&registry, &req("claude-opus-4-7"))
        .expect("explicit route must match");
    assert_eq!(resolved.provider.as_str(), "anthropic");
}

#[test]
fn resolve_route_falls_back_to_default_provider() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let resolved = config
        .resolve_route(&registry, &req("some-unknown-model"))
        .expect("default provider must absorb unmatched model");
    assert_eq!(resolved.provider.as_str(), "gemini");
    assert!(
        resolved.upstream_model.is_none(),
        "the synthesized default route must not carry an upstream_model rewrite"
    );
    // The synthetic default route forwards the requested model verbatim;
    // per-model upstream rewrites are applied downstream from the registry.
    assert_eq!(
        resolved.effective_upstream_model("some-unknown-model"),
        "some-unknown-model",
        "synthetic default route must pass the requested model through unchanged"
    );
}

#[test]
fn resolve_route_is_none_without_default_or_match() {
    let config = two_provider_config(None);
    let registry = two_provider_registry();
    assert!(
        config
            .resolve_route(&registry, &req("some-unknown-model"))
            .is_none()
    );
}

#[test]
fn is_model_exposed_is_closed_by_default_even_with_default_provider() {
    let registry = two_provider_registry();
    assert!(
        !two_provider_config(None).is_model_exposed(&registry, "some-unknown-model"),
        "closed gateway must deny unknown models"
    );
    // A default provider authorizes the synthetic catch-all route, but it does
    // NOT, on its own, expose an unlisted model to dispatch; the gateway stays
    // a closed allowlist unless allow_unlisted_models is set.
    assert!(
        !two_provider_config(Some("gemini")).is_model_exposed(&registry, "some-unknown-model"),
        "a default provider alone must not open the gateway to unlisted models"
    );
}

#[test]
fn is_model_exposed_opens_only_when_allow_unlisted_models() {
    let registry = two_provider_registry();
    let open = GatewayConfig {
        allow_unlisted_models: true,
        ..two_provider_config(Some("gemini"))
    };
    assert!(
        open.is_model_exposed(&registry, "some-unknown-model"),
        "allow_unlisted_models opts into forwarding unlisted models to default_provider"
    );
    // …but a routed model and a registry model are exposed regardless of the flag.
    assert!(open.is_model_exposed(&registry, "claude-sonnet-4-20250514"));
    let closed_no_default = GatewayConfig {
        allow_unlisted_models: true,
        ..two_provider_config(None)
    };
    assert!(
        !closed_no_default.is_model_exposed(&registry, "some-unknown-model"),
        "allow_unlisted_models without a default_provider still denies unknown models"
    );
}

#[test]
fn is_model_exposed_admits_registry_model() {
    let registry = two_provider_registry();
    assert!(
        two_provider_config(None).is_model_exposed(&registry, "claude-sonnet-4-20250514"),
        "a model present in the registry must be exposed even without a default provider"
    );
}

#[test]
fn validate_rejects_default_provider_absent_from_registry() {
    let registry = two_provider_registry();
    match two_provider_config(Some("openai")).validate(&registry) {
        Err(GatewayProfileError::DefaultProviderNotInRegistry { provider }) => {
            assert_eq!(provider, "openai");
        },
        other => panic!("expected DefaultProviderNotInRegistry, got {other:?}"),
    }
    assert!(
        two_provider_config(Some("gemini"))
            .validate(&registry)
            .is_ok(),
        "a default provider present in the registry must validate"
    );
}

fn route_id(route: Cow<'_, GatewayRoute>) -> RouteId {
    let mut route = route.into_owned();
    route.ensure_id();
    route.id
}

#[test]
fn dispatchable_route_ids_cover_every_candidate_route() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let ids = config.dispatchable_route_ids(&registry);

    for route in config.candidate_routes(&registry) {
        let id = route_id(route);
        assert!(
            ids.contains(&id),
            "candidate {id:?} absent from catalog {ids:?}"
        );
    }

    let resolved = config
        .resolve_route(&registry, &req("some-unknown-model"))
        .expect("default provider must absorb unmatched model");
    assert!(ids.contains(&route_id(resolved)));
}

#[test]
fn dispatchable_route_ids_omits_default_when_unset() {
    let registry = two_provider_registry();
    let ids = two_provider_config(None).dispatchable_route_ids(&registry);
    assert_eq!(ids.len(), 2);
}

#[test]
fn dispatchable_route_ids_dedupes_explicit_catch_all() {
    let mut config = two_provider_config(Some("gemini"));
    config.routes.push(route_to("*", "gemini"));
    let registry = two_provider_registry();
    let ids = config.dispatchable_route_ids(&registry);
    let mut unique = ids.clone();
    unique.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "route ids must be unique: {ids:?}");
}

#[test]
fn gateway_state_dispatchable_route_ids_resolves_spec() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let from_state = GatewayState::Spec(config.to_spec()).dispatchable_route_ids(&registry);
    assert_eq!(from_state, config.dispatchable_route_ids(&registry));
}

#[test]
fn gateway_spec_round_trips_default_provider() {
    let spec = two_provider_config(Some("gemini")).to_spec();
    let yaml = serde_yaml::to_string(&spec).expect("serialize");
    assert!(yaml.contains("default_provider: gemini"), "got:\n{yaml}");

    let back: GatewayConfigSpec = serde_yaml::from_str(&yaml).expect("round-trip");
    assert_eq!(
        back.default_provider.as_ref().map(ProviderId::as_str),
        Some("gemini")
    );
}

#[test]
fn exact_pattern_does_not_match_suffixed_alias() {
    assert!(!route("gpt-5.4").matches("gpt-5.4-mini"));
    assert!(route("gpt-*").matches("gpt-5.4-mini"));
}

#[test]
fn gpt_star_route_exposes_and_rewrites_codex_alias() {
    let registry = ProviderRegistry {
        providers: vec![provider_entry(
            "openai",
            "https://api.openai.com/v1",
            vec![model("gpt-5-mini")],
        )],
    };
    let mut openai = route_to("gpt-*", "openai");
    openai.upstream_model = Some("gpt-5-mini".to_owned());
    let config = GatewayConfig {
        enabled: true,
        routes: vec![openai],
        default_provider: Some(ProviderId::new("openai")),
        ..GatewayConfig::default()
    };

    assert!(
        config.is_model_exposed(&registry, "gpt-5.4-mini"),
        "a gpt-* route must expose Codex's gpt-5.4-mini alias"
    );
    let resolved = config
        .resolve_route(&registry, &req("gpt-5.4-mini"))
        .expect("gpt-* route must resolve gpt-5.4-mini");
    assert_eq!(resolved.provider.as_str(), "openai");
    assert_eq!(
        resolved.effective_upstream_model("gpt-5.4-mini"),
        "gpt-5-mini",
        "the alias must be rewritten to the concrete upstream model OpenAI accepts"
    );
}

#[test]
fn default_resource_audiences_cover_gateway_requirements() {
    let audiences = default_resource_audiences();
    assert!(audiences.contains(&"hook".to_owned()));
    assert_eq!(audiences.len(), 1);
}

#[test]
fn system_prompt_rule_round_trips_under_deny_unknown_fields() {
    let yaml = "provider: cerebras\nmodel_pattern: claude-*\naction: replace\nprompt: hi\n";
    let rule: SystemPromptRule = serde_yaml::from_str(yaml).expect("rule must deserialize");
    assert_eq!(rule.action, OverrideRuleAction::Replace);
    assert!(rule.matches(&ProviderId::new("cerebras"), "claude-opus-4-8"));
    assert!(!rule.matches(&ProviderId::new("openai"), "claude-opus-4-8"));
    assert!(serde_yaml::from_str::<SystemPromptRule>("action: strip\nbogus: 1\n").is_err());
}

#[test]
fn system_prompt_rule_matches_are_optional() {
    let any = SystemPromptRule {
        provider: None,
        model_pattern: None,
        action: OverrideRuleAction::Strip,
        prompt: None,
    };
    assert!(any.matches(&ProviderId::new("anything"), "any-model"));
}

#[test]
fn validate_rejects_replace_without_prompt() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![route_to("claude-*", "anthropic")],
        system_prompt_overrides: vec![SystemPromptRule {
            provider: Some(ProviderId::new("anthropic")),
            model_pattern: None,
            action: OverrideRuleAction::Replace,
            prompt: None,
        }],
        ..GatewayConfig::default()
    };
    assert!(matches!(
        config.validate(&two_provider_registry()),
        Err(GatewayProfileError::OverrideReplaceMissingPrompt)
    ));
}

#[test]
fn validate_rejects_strip_with_prompt() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![route_to("claude-*", "anthropic")],
        system_prompt_overrides: vec![SystemPromptRule {
            provider: None,
            model_pattern: None,
            action: OverrideRuleAction::Strip,
            prompt: Some("nope".to_owned()),
        }],
        ..GatewayConfig::default()
    };
    assert!(matches!(
        config.validate(&two_provider_registry()),
        Err(GatewayProfileError::OverrideStripWithPrompt)
    ));
}

#[test]
fn validate_rejects_override_unknown_provider() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![route_to("claude-*", "anthropic")],
        system_prompt_overrides: vec![SystemPromptRule {
            provider: Some(ProviderId::new("ghost")),
            model_pattern: None,
            action: OverrideRuleAction::Strip,
            prompt: None,
        }],
        ..GatewayConfig::default()
    };
    assert!(matches!(
        config.validate(&two_provider_registry()),
        Err(GatewayProfileError::OverrideProviderNotInRegistry { provider }) if provider == "ghost"
    ));
}

fn route_when(pattern: &str, provider: &str, when: RouteMatch) -> GatewayRoute {
    let mut r = route_to(pattern, provider);
    r.when = Some(when);
    r
}

fn thinking_request(model: &str) -> CanonicalRequest {
    let mut r = req(model);
    r.thinking = Some(ThinkingConfig {
        enabled: true,
        budget_tokens: None,
    });
    r
}

#[test]
fn route_without_when_resolves_on_model_only() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![route_to("claude-*", "anthropic")],
        ..GatewayConfig::default()
    };
    let registry = two_provider_registry();
    let plain = config
        .resolve_route(&registry, &req("claude-sonnet-4-20250514"))
        .expect("model-only route must match plain request");
    let thinking = config
        .resolve_route(&registry, &thinking_request("claude-sonnet-4-20250514"))
        .expect("model-only route must match thinking request");
    assert_eq!(plain.provider.as_str(), "anthropic");
    assert_eq!(
        thinking.provider.as_str(),
        "anthropic",
        "a when-less route must ignore request shape (behaviour unchanged)"
    );
}

#[test]
fn when_thinking_splits_traffic_with_catch_all_fallback() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![
            route_when(
                "claude-*",
                "anthropic",
                RouteMatch {
                    thinking: Some(true),
                    ..RouteMatch::default()
                },
            ),
            route_to("claude-*", "gemini"),
        ],
        ..GatewayConfig::default()
    };
    let registry = two_provider_registry();
    let thinking = config
        .resolve_route(&registry, &thinking_request("claude-sonnet-4-20250514"))
        .expect("thinking request must match the first route");
    let plain = config
        .resolve_route(&registry, &req("claude-sonnet-4-20250514"))
        .expect("non-thinking request must fall through to the catch-all");
    assert_eq!(thinking.provider.as_str(), "anthropic");
    assert_eq!(plain.provider.as_str(), "gemini");
}

#[test]
fn route_match_predicates_evaluate_against_request() {
    let mut effort = req("m");
    effort.reasoning_effort = Some(ReasoningEffort::Medium);
    let floor_high = RouteMatch {
        min_reasoning_effort: Some(ReasoningEffort::High),
        ..RouteMatch::default()
    };
    let floor_low = RouteMatch {
        min_reasoning_effort: Some(ReasoningEffort::Low),
        ..RouteMatch::default()
    };
    assert!(!floor_high.matches_request(&effort));
    assert!(floor_low.matches_request(&effort));

    let mut streamed = req("m");
    streamed.stream = true;
    assert!(
        RouteMatch {
            stream: Some(true),
            ..RouteMatch::default()
        }
        .matches_request(&streamed)
    );
    assert!(
        !RouteMatch {
            stream: Some(false),
            ..RouteMatch::default()
        }
        .matches_request(&streamed)
    );

    let mut tooled = req("m");
    tooled.tools = vec![CanonicalTool {
        name: "t".to_owned(),
        description: None,
        input_schema: serde_json::Value::Null,
    }];
    assert!(
        RouteMatch {
            requires_tools: Some(true),
            min_tools: Some(1),
            ..RouteMatch::default()
        }
        .matches_request(&tooled)
    );
    assert!(
        !RouteMatch {
            min_tools: Some(2),
            ..RouteMatch::default()
        }
        .matches_request(&tooled)
    );
}

#[test]
fn route_match_min_input_tokens_uses_text_estimate() {
    let mut r = req("m");
    r.system = Some("a".repeat(40));
    r.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::Text("b".repeat(40))],
    }];
    // ~80 chars / 4 + 1 ≈ 21 estimated tokens.
    assert!(
        RouteMatch {
            min_input_tokens: Some(20),
            ..RouteMatch::default()
        }
        .matches_request(&r)
    );
    assert!(
        !RouteMatch {
            min_input_tokens: Some(100),
            ..RouteMatch::default()
        }
        .matches_request(&r)
    );
}

#[test]
fn route_match_response_format_discriminates() {
    let mut json = req("m");
    json.response_format = Some(ResponseFormat::JsonObject);
    assert!(
        RouteMatch {
            response_format: Some(ResponseFormatKind::JsonObject),
            ..RouteMatch::default()
        }
        .matches_request(&json)
    );
    assert!(
        RouteMatch {
            response_format: Some(ResponseFormatKind::Text),
            ..RouteMatch::default()
        }
        .matches_request(&req("m")),
        "an absent wire response_format reads as Text"
    );
}

#[test]
fn when_block_rejects_unknown_fields() {
    let yaml = "model_pattern: claude-*\nprovider: anthropic\nwhen:\n  bogus: true\n";
    assert!(serde_yaml::from_str::<GatewayRoute>(yaml).is_err());
}

#[test]
fn route_match_validate_rejects_nonsense() {
    assert!(matches!(
        RouteMatch {
            min_tools: Some(0),
            ..RouteMatch::default()
        }
        .validate(),
        Err(GatewayProfileError::RouteMatchZeroMinTools)
    ));
    assert!(matches!(
        RouteMatch {
            requires_tools: Some(false),
            min_tools: Some(3),
            ..RouteMatch::default()
        }
        .validate(),
        Err(GatewayProfileError::RouteMatchContradictoryTools)
    ));
    assert!(RouteMatch::default().validate().is_ok());
}

#[test]
fn reasoning_effort_orders_and_round_trips_snake_case() {
    assert!(ReasoningEffort::Low < ReasoningEffort::Medium);
    assert!(ReasoningEffort::Medium < ReasoningEffort::High);
    assert_eq!(
        serde_json::to_string(&ReasoningEffort::High).expect("serialize"),
        "\"high\""
    );
    assert_eq!(
        serde_json::from_str::<ReasoningEffort>("\"low\"").expect("deserialize"),
        ReasoningEffort::Low
    );
}

#[test]
fn matched_predicates_lists_set_fields_in_declaration_order() {
    assert!(RouteMatch::default().matched_predicates().is_empty());

    let full = RouteMatch {
        requires_tools: Some(true),
        min_tools: Some(2),
        thinking: Some(true),
        min_reasoning_effort: Some(ReasoningEffort::Low),
        stream: Some(true),
        min_input_tokens: Some(10),
        response_format: Some(ResponseFormatKind::JsonObject),
    };
    assert_eq!(
        full.matched_predicates(),
        vec![
            "requires_tools",
            "min_tools",
            "thinking",
            "min_reasoning_effort",
            "stream",
            "min_input_tokens",
            "response_format",
        ]
    );

    let sparse = RouteMatch {
        thinking: Some(false),
        min_input_tokens: Some(1),
        ..RouteMatch::default()
    };
    assert_eq!(
        sparse.matched_predicates(),
        vec!["thinking", "min_input_tokens"]
    );
}

#[test]
fn route_match_validate_rejects_vacuous_and_contradictory_tool_predicates() {
    assert!(
        RouteMatch {
            min_tools: Some(0),
            ..RouteMatch::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        RouteMatch {
            requires_tools: Some(false),
            min_tools: Some(1),
            ..RouteMatch::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        RouteMatch {
            requires_tools: Some(true),
            min_tools: Some(1),
            ..RouteMatch::default()
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn route_match_token_estimate_counts_thinking_and_nested_tool_result_text() {
    let mut r = req("m");
    r.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![
            CanonicalContent::Thinking {
                text: "x".repeat(40),
                signature: None,
            },
            CanonicalContent::ToolResult {
                tool_use_id: "call_1".to_owned(),
                content: vec![CanonicalContent::Text("y".repeat(40))],
                is_error: false,
                structured_content: None,
                meta: None,
            },
        ],
    }];
    assert!(
        RouteMatch {
            min_input_tokens: Some(20),
            ..RouteMatch::default()
        }
        .matches_request(&r),
        "thinking and tool-result text must contribute to the estimate"
    );
}

#[test]
fn route_resolve_finds_registry_entry_by_provider_name() {
    let registry = registry_with_endpoint("https://api.example.com/v1");
    let hit = route("claude-*");
    assert!(hit.resolve(&registry).is_some());

    let mut miss = route("claude-*");
    miss.provider = ProviderId::new("absent");
    assert!(miss.resolve(&registry).is_none());
}
