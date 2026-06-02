use systemprompt_models::validators::{
    AiConfigValidator, ContentConfigValidator, McpConfigValidator, RateLimitsConfigValidator,
    ValidationConfigProvider, WebConfigRaw, WebConfigValidator,
};
use systemprompt_models::{Config, ContentConfigRaw, ServicesConfig};
use systemprompt_test_fixtures::fixture_config;
use systemprompt_traits::DomainConfig;

fn base_config() -> Config {
    fixture_config("postgres://localhost/test")
}

fn provider(config: Config, services: ServicesConfig) -> ValidationConfigProvider {
    ValidationConfigProvider::new(config, services)
}

fn services_with_ai_json(ai: serde_json::Value) -> ServicesConfig {
    let json = serde_json::json!({ "ai": ai });
    serde_json::from_value(json).expect("ServicesConfig deserializes")
}

mod ai_validator {
    use super::*;

    #[test]
    fn errors_when_default_provider_empty() {
        let services = ServicesConfig::default();
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn errors_when_default_provider_not_in_providers() {
        let services = services_with_ai_json(serde_json::json!({
            "default_provider": "ghost",
            "providers": {
                "anthropic": { "enabled": true, "default_model": "claude" }
            }
        }));
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn errors_when_no_providers_enabled() {
        let services = services_with_ai_json(serde_json::json!({
            "default_provider": "anthropic",
            "providers": {
                "anthropic": { "enabled": false, "default_model": "claude" }
            }
        }));
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn valid_config_has_no_errors() {
        let services = services_with_ai_json(serde_json::json!({
            "default_provider": "anthropic",
            "providers": {
                "anthropic": { "enabled": true, "default_model": "claude-3" }
            }
        }));
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_errors(), "report: {report:?}");
    }

    #[test]
    fn warns_when_enabled_provider_has_no_default_model() {
        let services = services_with_ai_json(serde_json::json!({
            "default_provider": "anthropic",
            "providers": {
                "anthropic": { "enabled": true, "default_model": "" }
            }
        }));
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_errors());
        assert!(report.has_warnings());
    }

    #[test]
    fn warns_when_history_retention_zero() {
        let services = services_with_ai_json(serde_json::json!({
            "default_provider": "anthropic",
            "providers": {
                "anthropic": { "enabled": true, "default_model": "claude-3" }
            },
            "history": { "retention_days": 0 }
        }));
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_warnings());
    }

    #[test]
    fn errors_when_mcp_connect_timeout_zero() {
        let services = services_with_ai_json(serde_json::json!({
            "default_provider": "anthropic",
            "providers": {
                "anthropic": { "enabled": true, "default_model": "claude-3" }
            },
            "mcp": { "resilience": { "connect_timeout_ms": 0, "request_timeout_ms": 1000 } }
        }));
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn errors_when_mcp_request_timeout_zero() {
        let services = services_with_ai_json(serde_json::json!({
            "default_provider": "anthropic",
            "providers": {
                "anthropic": { "enabled": true, "default_model": "claude-3" }
            },
            "mcp": { "resilience": { "connect_timeout_ms": 1000, "request_timeout_ms": 0 } }
        }));
        let prov = provider(base_config(), services);
        let mut v = AiConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn load_rejects_wrong_provider_type() {
        let config = base_config();
        let mut v = AiConfigValidator::new();
        let result = v.load(&config);
        assert!(result.is_err());
    }
}

mod rate_limits_validator {
    use super::*;

    fn config_with_rate_limits(
        f: impl FnOnce(&mut systemprompt_models::config::RateLimitConfig),
    ) -> Config {
        let mut config = base_config();
        config.rate_limits.disabled = false;
        f(&mut config.rate_limits);
        config
    }

    #[test]
    fn disabled_config_short_circuits_with_no_warnings() {
        let mut config = base_config();
        config.rate_limits.disabled = true;
        let prov = provider(config, ServicesConfig::default());
        let mut v = RateLimitsConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_warnings());
        assert!(!report.has_errors());
    }

    #[test]
    fn warns_on_low_stream_per_second() {
        let config = config_with_rate_limits(|rl| {
            rl.stream_per_second = 1;
            rl.agents_per_second = 100;
            rl.contexts_per_second = 100;
            rl.tier_multipliers.anon = 1.0;
            rl.tier_multipliers.user = 1.0;
        });
        let prov = provider(config, ServicesConfig::default());
        let mut v = RateLimitsConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_warnings());
    }

    #[test]
    fn warns_on_low_tier_multipliers() {
        let config = config_with_rate_limits(|rl| {
            rl.stream_per_second = 100;
            rl.agents_per_second = 100;
            rl.contexts_per_second = 100;
            rl.tier_multipliers.anon = 0.1;
            rl.tier_multipliers.user = 0.1;
        });
        let prov = provider(config, ServicesConfig::default());
        let mut v = RateLimitsConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_warnings());
    }

    #[test]
    fn warns_on_low_agent_and_context_limits() {
        let config = config_with_rate_limits(|rl| {
            rl.stream_per_second = 100;
            rl.agents_per_second = 1;
            rl.contexts_per_second = 1;
            rl.tier_multipliers.anon = 1.0;
            rl.tier_multipliers.user = 1.0;
        });
        let prov = provider(config, ServicesConfig::default());
        let mut v = RateLimitsConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_warnings());
    }

    #[test]
    fn healthy_limits_produce_no_warnings() {
        let config = config_with_rate_limits(|rl| {
            rl.stream_per_second = 100;
            rl.agents_per_second = 100;
            rl.contexts_per_second = 100;
            rl.tier_multipliers.anon = 1.0;
            rl.tier_multipliers.user = 1.0;
        });
        let prov = provider(config, ServicesConfig::default());
        let mut v = RateLimitsConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_warnings());
    }
}

mod mcp_validator {
    use super::*;

    #[test]
    fn empty_servers_produce_no_errors() {
        let prov = provider(base_config(), ServicesConfig::default());
        let mut v = McpConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_errors());
    }

    #[test]
    fn load_rejects_wrong_provider_type() {
        let config = base_config();
        let mut v = McpConfigValidator::new();
        assert!(v.load(&config).is_err());
    }
}

mod content_validator {
    use super::*;

    #[test]
    fn no_content_config_short_circuits_ok() {
        let prov = provider(base_config(), ServicesConfig::default());
        let mut v = ContentConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_errors());
    }

    #[test]
    fn empty_content_sources_produce_no_errors() {
        let raw: ContentConfigRaw =
            serde_json::from_value(serde_json::json!({})).expect("empty content config");
        let prov = provider(base_config(), ServicesConfig::default()).with_content_config(raw);
        let mut v = ContentConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_errors());
    }

    #[test]
    fn missing_source_directory_is_reported() {
        let raw: ContentConfigRaw = serde_json::from_value(serde_json::json!({
            "content_sources": {
                "blog": {
                    "path": "does/not/exist",
                    "source_id": "blog",
                    "category_id": "writing",
                    "enabled": true
                }
            },
            "categories": {
                "writing": { "name": "Writing" }
            }
        }))
        .expect("content config with sources");
        let prov = provider(base_config(), ServicesConfig::default()).with_content_config(raw);
        let mut v = ContentConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn unknown_category_reference_is_reported() {
        let raw: ContentConfigRaw = serde_json::from_value(serde_json::json!({
            "content_sources": {
                "blog": {
                    "path": "does/not/exist",
                    "source_id": "blog",
                    "category_id": "ghost",
                    "enabled": true
                }
            },
            "categories": {}
        }))
        .expect("content config");
        let prov = provider(base_config(), ServicesConfig::default()).with_content_config(raw);
        let mut v = ContentConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }
}

mod web_validator {
    use super::*;

    fn config_with_web_path(path: &str) -> Config {
        let mut config = base_config();
        config.web_config_path = path.to_owned();
        config
    }

    fn full_branding() -> serde_json::Value {
        serde_json::json!({
            "copyright": "© 2024 Co",
            "twitter_handle": "@co",
            "display_sitename": true,
            "favicon": "/favicon.ico",
            "logo": { "primary": { "svg": "/logo.svg" } }
        })
    }

    #[test]
    fn no_web_config_short_circuits_ok() {
        let prov = provider(base_config(), ServicesConfig::default());
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_errors());
    }

    #[test]
    fn invalid_base_url_is_reported() {
        let raw: WebConfigRaw = serde_json::from_value(serde_json::json!({
            "base_url": "ftp://example.com",
            "branding": full_branding()
        }))
        .expect("web config");
        let prov = provider(
            config_with_web_path("/tmp/web.yaml"),
            ServicesConfig::default(),
        )
        .with_web_config(raw);
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn empty_site_name_is_reported() {
        let raw: WebConfigRaw = serde_json::from_value(serde_json::json!({
            "site_name": "",
            "branding": full_branding()
        }))
        .expect("web config");
        let prov = provider(
            config_with_web_path("/tmp/web.yaml"),
            ServicesConfig::default(),
        )
        .with_web_config(raw);
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn missing_branding_is_reported() {
        let raw: WebConfigRaw = serde_json::from_value(serde_json::json!({
            "site_name": "Site",
            "base_url": "https://example.com"
        }))
        .expect("web config");
        let prov = provider(
            config_with_web_path("/tmp/web.yaml"),
            ServicesConfig::default(),
        )
        .with_web_config(raw);
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn partial_branding_reports_each_missing_field() {
        let raw: WebConfigRaw = serde_json::from_value(serde_json::json!({
            "site_name": "Site",
            "base_url": "https://example.com",
            "branding": { "copyright": "© 2024" }
        }))
        .expect("web config");
        let prov = provider(
            config_with_web_path("/tmp/web.yaml"),
            ServicesConfig::default(),
        )
        .with_web_config(raw);
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_errors());
    }

    #[test]
    fn valid_web_config_with_full_branding_has_no_errors() {
        let raw: WebConfigRaw = serde_json::from_value(serde_json::json!({
            "site_name": "Site",
            "base_url": "https://example.com",
            "branding": full_branding(),
            "paths": {}
        }))
        .expect("web config");
        let prov = provider(
            config_with_web_path("/tmp/web.yaml"),
            ServicesConfig::default(),
        )
        .with_web_config(raw);
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(!report.has_errors(), "report: {report:?}");
    }

    #[test]
    fn missing_paths_section_produces_warning() {
        let raw: WebConfigRaw = serde_json::from_value(serde_json::json!({
            "site_name": "Site",
            "base_url": "https://example.com",
            "branding": full_branding()
        }))
        .expect("web config");
        let prov = provider(
            config_with_web_path("/tmp/web.yaml"),
            ServicesConfig::default(),
        )
        .with_web_config(raw);
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        let report = v.validate().expect("validate");
        assert!(report.has_warnings());
    }

    #[test]
    fn load_rejects_wrong_provider_type() {
        let config = base_config();
        let mut v = WebConfigValidator::new();
        assert!(v.load(&config).is_err());
    }
}
