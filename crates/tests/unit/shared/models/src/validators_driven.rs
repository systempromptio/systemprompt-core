use systemprompt_models::validators::{
    AgentConfigValidator, AiConfigValidator, ContentConfigValidator, McpConfigValidator,
    RateLimitsConfigValidator, ValidationConfigProvider, WebConfigRaw, WebConfigValidator,
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

mod agents_validator {
    use super::*;

    fn agent_json(name: &str, port: u16, extra: serde_json::Value) -> serde_json::Value {
        let mut agent = serde_json::json!({
            "name": name,
            "port": port,
            "endpoint": format!("http://127.0.0.1:{port}"),
            "enabled": true,
            "card": {
                "protocolVersion": "0.2.5",
                "displayName": "Agent",
                "description": "d",
                "version": "1.0.0"
            },
            "metadata": {}
        });
        if let (Some(obj), Some(extra_obj)) = (agent.as_object_mut(), extra.as_object()) {
            for (k, v) in extra_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
        agent
    }

    fn services_with(agents: serde_json::Value, mcp_servers: serde_json::Value) -> ServicesConfig {
        serde_json::from_value(serde_json::json!({
            "agents": agents,
            "mcp_servers": mcp_servers
        }))
        .expect("ServicesConfig deserializes")
    }

    fn validated(
        services: ServicesConfig,
    ) -> systemprompt_traits::validation_report::ValidationReport {
        let prov = provider(base_config(), services);
        let mut v = AgentConfigValidator::new();
        v.load(&prov).expect("load");
        v.validate().expect("validate")
    }

    #[test]
    fn load_rejects_wrong_provider_type() {
        let config = base_config();
        let mut v = AgentConfigValidator::new();
        assert!(v.load(&config).is_err());
    }

    #[test]
    fn validate_without_load_errors() {
        let v = AgentConfigValidator::new();
        assert!(v.validate().is_err());
    }

    #[test]
    fn duplicate_ports_are_reported() {
        let services = services_with(
            serde_json::json!({
                "one": agent_json("one", 5001, serde_json::json!({})),
                "two": agent_json("two", 5001, serde_json::json!({}))
            }),
            serde_json::json!({}),
        );
        assert!(validated(services).has_errors());
    }

    #[test]
    fn unique_ports_and_no_refs_pass() {
        let services = services_with(
            serde_json::json!({
                "one": agent_json("one", 5001, serde_json::json!({})),
                "two": agent_json("two", 5002, serde_json::json!({}))
            }),
            serde_json::json!({}),
        );
        let report = validated(services);
        assert!(!report.has_errors(), "report: {report:?}");
    }

    #[test]
    fn empty_agent_name_is_reported() {
        let services = services_with(
            serde_json::json!({ "one": agent_json("", 5001, serde_json::json!({})) }),
            serde_json::json!({}),
        );
        assert!(validated(services).has_errors());
    }

    #[test]
    fn missing_skill_directory_is_reported() {
        let services = services_with(
            serde_json::json!({
                "one": agent_json("one", 5001, serde_json::json!({
                    "metadata": { "skills": { "include": ["ghost_skill_does_not_exist"] } }
                }))
            }),
            serde_json::json!({}),
        );
        assert!(validated(services).has_errors());
    }

    #[test]
    fn undefined_mcp_server_reference_is_reported() {
        let services = services_with(
            serde_json::json!({
                "one": agent_json("one", 5001, serde_json::json!({
                    "metadata": { "mcpServers": { "include": ["missing_server"] } }
                }))
            }),
            serde_json::json!({}),
        );
        assert!(validated(services).has_errors());
    }

    #[test]
    fn production_agent_referencing_dev_only_mcp_is_reported() {
        let mcp = serde_json::json!({
            "core": {
                "binary": "core-mcp",
                "package": null,
                "port": 5080,
                "enabled": true,
                "display_in_web": false,
                "dev_only": true,
                "oauth": { "required": false, "scopes": [], "audience": "mcp", "client_id": null }
            }
        });
        let services = services_with(
            serde_json::json!({
                "one": agent_json("one", 5001, serde_json::json!({
                    "metadata": { "mcpServers": { "include": ["core"] } }
                }))
            }),
            mcp.clone(),
        );
        assert!(validated(services).has_errors());

        let dev_services = services_with(
            serde_json::json!({
                "one": agent_json("one", 5001, serde_json::json!({
                    "dev_only": true,
                    "metadata": { "mcpServers": { "include": ["core"] } }
                }))
            }),
            mcp,
        );
        let report = validated(dev_services);
        assert!(!report.has_errors(), "report: {report:?}");
    }
}

mod web_validator_paths {
    use super::*;

    fn web_raw(paths: serde_json::Value) -> WebConfigRaw {
        serde_json::from_value(serde_json::json!({
            "site_name": "Site",
            "base_url": "https://example.com",
            "branding": {
                "copyright": "© 2024 Co",
                "twitter_handle": "@co",
                "display_sitename": true,
                "favicon": "/favicon.ico",
                "logo": { "primary": { "svg": "/logo.svg" } }
            },
            "paths": paths
        }))
        .expect("web config")
    }

    fn validated(
        config: Config,
        raw: WebConfigRaw,
    ) -> systemprompt_traits::validation_report::ValidationReport {
        let prov = provider(config, ServicesConfig::default()).with_web_config(raw);
        let mut v = WebConfigValidator::new();
        v.load(&prov).expect("load");
        v.validate().expect("validate")
    }

    #[test]
    fn missing_templates_directory_is_reported() {
        let mut config = base_config();
        config.web_config_path = "/tmp/web.yaml".to_owned();
        let raw = web_raw(serde_json::json!({ "templates": "/nonexistent/templates/dir" }));
        assert!(validated(config, raw).has_errors());
    }

    #[test]
    fn templates_path_that_is_a_file_is_reported() {
        let dir = std::env::temp_dir().join(format!("web-tpl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = dir.join("templates");
        std::fs::write(&file, b"x").expect("write");

        let mut config = base_config();
        config.web_config_path = "/tmp/web.yaml".to_owned();
        let raw = web_raw(serde_json::json!({ "templates": file.to_string_lossy() }));
        let report = validated(config, raw);
        std::fs::remove_dir_all(&dir).ok();
        assert!(report.has_errors());
    }

    #[test]
    fn relative_templates_path_resolves_against_system_path() {
        let base = std::env::temp_dir().join(format!("web-sys-{}", std::process::id()));
        std::fs::create_dir_all(base.join("templates")).expect("mkdir");

        let mut config = base_config();
        config.web_config_path = "/tmp/web.yaml".to_owned();
        config.system_path = base.to_string_lossy().to_string();
        let raw = web_raw(serde_json::json!({ "templates": "templates" }));
        let report = validated(config, raw);
        std::fs::remove_dir_all(&base).ok();
        assert!(!report.has_errors(), "report: {report:?}");
    }

    #[test]
    fn nonexistent_config_directory_is_reported() {
        let mut config = base_config();
        config.web_config_path = "/nonexistent-dir-xyz/web.yaml".to_owned();
        let raw = web_raw(serde_json::json!({}));
        assert!(validated(config, raw).has_errors());
    }
}
