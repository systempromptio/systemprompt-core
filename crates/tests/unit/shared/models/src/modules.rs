use systemprompt_models::modules::{
    ApiConfig, ApiPaths, CliPaths, Module, ModulePermission, ModuleType, Modules, ServiceCategory,
};

mod service_category_tests {
    use super::*;

    #[test]
    fn core_base_path() {
        assert_eq!(ServiceCategory::Core.base_path(), "/api/v1/core");
    }

    #[test]
    fn agent_base_path() {
        assert_eq!(ServiceCategory::Agent.base_path(), "/api/v1/agents");
    }

    #[test]
    fn mcp_base_path() {
        assert_eq!(ServiceCategory::Mcp.base_path(), "/api/v1/mcp");
    }

    #[test]
    fn meta_base_path() {
        assert_eq!(ServiceCategory::Meta.base_path(), "/");
    }

    #[test]
    fn display_names() {
        assert_eq!(ServiceCategory::Core.display_name(), "Core");
        assert_eq!(ServiceCategory::Agent.display_name(), "Agent");
        assert_eq!(ServiceCategory::Mcp.display_name(), "MCP");
        assert_eq!(ServiceCategory::Meta.display_name(), "Meta");
    }

    #[test]
    fn mount_path_with_module_name() {
        assert_eq!(
            ServiceCategory::Core.mount_path("users"),
            "/api/v1/core/users"
        );
    }

    #[test]
    fn mount_path_agent_with_module_name() {
        assert_eq!(
            ServiceCategory::Agent.mount_path("assistant"),
            "/api/v1/agents/assistant"
        );
    }

    #[test]
    fn mount_path_meta_with_module_name() {
        assert_eq!(ServiceCategory::Meta.mount_path("health"), "/health");
    }

    #[test]
    fn mount_path_empty_module_name_returns_base() {
        assert_eq!(ServiceCategory::Core.mount_path(""), "/api/v1/core");
    }

    #[test]
    fn matches_path_core() {
        assert!(ServiceCategory::Core.matches_path("/api/v1/core/contexts"));
        assert!(!ServiceCategory::Core.matches_path("/api/v1/agents/foo"));
    }

    #[test]
    fn matches_path_meta_root() {
        assert!(ServiceCategory::Meta.matches_path("/"));
    }

    #[test]
    fn matches_path_meta_wellknown() {
        assert!(ServiceCategory::Meta.matches_path("/.well-known/agent-card.json"));
    }

    #[test]
    fn matches_path_meta_api_meta() {
        assert!(ServiceCategory::Meta.matches_path("/api/v1/meta/something"));
    }

    #[test]
    fn all_returns_four_categories() {
        let all = ServiceCategory::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn from_path_core() {
        assert_eq!(
            ServiceCategory::from_path("/api/v1/core/contexts"),
            Some(ServiceCategory::Core)
        );
    }

    #[test]
    fn from_path_agent() {
        assert_eq!(
            ServiceCategory::from_path("/api/v1/agents/registry"),
            Some(ServiceCategory::Agent)
        );
    }

    #[test]
    fn from_path_mcp() {
        assert_eq!(
            ServiceCategory::from_path("/api/v1/mcp/registry"),
            Some(ServiceCategory::Mcp)
        );
    }

    #[test]
    fn from_path_meta_wellknown() {
        assert_eq!(
            ServiceCategory::from_path("/.well-known/agent-card.json"),
            Some(ServiceCategory::Meta)
        );
    }

    #[test]
    fn from_path_unknown_returns_none() {
        assert_eq!(ServiceCategory::from_path("/totally/random/path"), None);
    }

    #[test]
    fn service_category_serde_roundtrip() {
        let category = ServiceCategory::Core;
        let json = serde_json::to_string(&category).unwrap();
        let deserialized: ServiceCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(category, deserialized);
    }
}

mod api_paths_tests {
    use super::*;

    #[test]
    fn tenant_path() {
        assert_eq!(ApiPaths::tenant("t1"), "/api/v1/tenants/t1");
    }

    #[test]
    fn tenant_status_path() {
        assert_eq!(ApiPaths::tenant_status("t1"), "/api/v1/tenants/t1/status");
    }

    #[test]
    fn tenant_deploy_path() {
        assert_eq!(ApiPaths::tenant_deploy("t1"), "/api/v1/tenants/t1/deploy");
    }

    #[test]
    fn mcp_server_endpoint_path() {
        assert_eq!(
            ApiPaths::mcp_server_endpoint("my-server"),
            "/api/v1/mcp/my-server/mcp"
        );
    }

    #[test]
    fn oauth_client_location_path() {
        assert_eq!(
            ApiPaths::oauth_client_location("client-123"),
            "/api/v1/core/oauth/clients/client-123"
        );
    }

    #[test]
    fn wellknown_agent_card_named_path() {
        assert_eq!(
            ApiPaths::wellknown_agent_card_named("my-agent"),
            "/.well-known/agent-cards/my-agent"
        );
    }

    #[test]
    fn agent_endpoint_path() {
        assert_eq!(
            ApiPaths::agent_endpoint("agent-1"),
            "/api/v1/agents/agent-1/"
        );
    }

    #[test]
    fn tenant_custom_domain_path() {
        assert_eq!(
            ApiPaths::tenant_custom_domain("t1"),
            "/api/v1/tenants/t1/custom-domain"
        );
    }

    #[test]
    fn static_constants_defined() {
        assert_eq!(ApiPaths::API_BASE, "/api");
        assert_eq!(ApiPaths::HEALTH, "/api/v1/health");
        assert_eq!(ApiPaths::OAUTH_TOKEN, "/api/v1/core/oauth/token");
        assert_eq!(ApiPaths::WELLKNOWN_AGENT_CARD, "/.well-known/agent-card.json");
    }
}

mod cli_paths_tests {
    use super::*;

    #[test]
    fn agent_run_args() {
        let args = CliPaths::agent_run_args();
        assert_eq!(args, ["admin", "agents", "run"]);
    }

    #[test]
    fn db_migrate_args() {
        let args = CliPaths::db_migrate_args();
        assert_eq!(args, ["infra", "db", "migrate"]);
    }

    #[test]
    fn services_serve_args() {
        let args = CliPaths::services_serve_args();
        assert_eq!(args, ["infra", "services", "serve"]);
    }

    #[test]
    fn db_migrate_cmd() {
        assert_eq!(CliPaths::db_migrate_cmd(), "infra db migrate");
    }

    #[test]
    fn services_serve_cmd() {
        assert_eq!(CliPaths::services_serve_cmd(), "infra services serve");
    }

    #[test]
    fn agent_run_cmd_pattern() {
        assert_eq!(CliPaths::agent_run_cmd_pattern(), "admin agents run");
    }

    #[test]
    fn infra_db_args() {
        let args = CliPaths::infra_db_args("status");
        assert_eq!(args, ["infra", "db", "status"]);
    }

    #[test]
    fn admin_agents_args() {
        let args = CliPaths::admin_agents_args("list");
        assert_eq!(args, ["admin", "agents", "list"]);
    }

    #[test]
    fn plugins_mcp_args() {
        let args = CliPaths::plugins_mcp_args("start");
        assert_eq!(args, ["plugins", "mcp", "start"]);
    }
}

mod module_type_tests {
    use super::*;

    #[test]
    fn module_type_regular_serde_roundtrip() {
        let mt = ModuleType::Regular;
        let json = serde_json::to_string(&mt).unwrap();
        let deserialized: ModuleType = serde_json::from_str(&json).unwrap();
        assert_eq!(mt, deserialized);
    }

    #[test]
    fn module_type_proxy_serde_roundtrip() {
        let mt = ModuleType::Proxy;
        let json = serde_json::to_string(&mt).unwrap();
        let deserialized: ModuleType = serde_json::from_str(&json).unwrap();
        assert_eq!(mt, deserialized);
    }

    #[test]
    fn module_type_eq() {
        assert_eq!(ModuleType::Regular, ModuleType::Regular);
        assert_ne!(ModuleType::Regular, ModuleType::Proxy);
    }
}

fn make_module(name: &str, deps: Vec<String>) -> Module {
    Module {
        uuid: "test-uuid".to_string(),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        display_name: name.to_string(),
        description: None,
        weight: None,
        dependencies: deps,
        schemas: None,
        seeds: None,
        permissions: None,
        audience: vec!["api".to_string()],
        enabled: true,
        api: None,
        path: std::path::PathBuf::from("/tmp"),
    }
}

mod modules_tests {
    use super::*;

    #[test]
    fn from_vec_empty() {
        let modules = Modules::from_vec(vec![]).unwrap();
        assert!(modules.all().is_empty());
    }

    #[test]
    fn from_vec_single_module() {
        let modules = Modules::from_vec(vec![make_module("core", vec![])]).unwrap();
        assert_eq!(modules.all().len(), 1);
    }

    #[test]
    fn get_existing_module() {
        let modules = Modules::from_vec(vec![make_module("core", vec![])]).unwrap();
        assert!(modules.get("core").is_some());
    }

    #[test]
    fn get_nonexistent_module() {
        let modules = Modules::from_vec(vec![make_module("core", vec![])]).unwrap();
        assert!(modules.get("missing").is_none());
    }

    #[test]
    fn list_names() {
        let modules = Modules::from_vec(vec![
            make_module("alpha", vec![]),
            make_module("beta", vec![]),
        ])
        .unwrap();
        let names = modules.list_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
    }

    #[test]
    fn provided_audiences() {
        let audiences = Modules::get_provided_audiences();
        assert!(audiences.contains(&"a2a".to_string()));
        assert!(audiences.contains(&"api".to_string()));
        assert!(audiences.contains(&"mcp".to_string()));
    }

    #[test]
    fn get_valid_audiences_for_existing_module() {
        let modules = Modules::from_vec(vec![make_module("core", vec![])]).unwrap();
        let audiences = modules.get_valid_audiences("core");
        assert_eq!(audiences, vec!["api".to_string()]);
    }

    #[test]
    fn get_valid_audiences_for_missing_module_returns_provided() {
        let modules = Modules::from_vec(vec![]).unwrap();
        let audiences = modules.get_valid_audiences("missing");
        assert_eq!(audiences.len(), 3);
    }

    #[test]
    fn resolve_dependencies_orders_correctly() {
        let modules = vec![
            make_module("b", vec!["a".to_string()]),
            make_module("a", vec![]),
        ];
        let ordered = Modules::resolve_dependencies(modules).unwrap();
        assert_eq!(ordered[0].name, "a");
        assert_eq!(ordered[1].name, "b");
    }

    #[test]
    fn resolve_dependencies_circular_fails() {
        let modules = vec![
            make_module("a", vec!["b".to_string()]),
            make_module("b", vec!["a".to_string()]),
        ];
        let result = Modules::resolve_dependencies(modules);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Circular dependency"));
    }

    #[test]
    fn resolve_dependencies_missing_dep_fails() {
        let modules = vec![make_module("a", vec!["nonexistent".to_string()])];
        let result = Modules::resolve_dependencies(modules);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Missing module dependencies"));
    }

    #[test]
    fn get_server_audiences() {
        let audiences = Modules::get_server_audiences("my-server", 8080);
        assert_eq!(audiences.len(), 3);
    }
}

mod module_permission_tests {
    use super::*;

    #[test]
    fn module_permission_serde_roundtrip() {
        let perm = ModulePermission {
            name: "read_users".to_string(),
            description: "Can read user data".to_string(),
            resource: "users".to_string(),
            action: "read".to_string(),
        };
        let json = serde_json::to_string(&perm).unwrap();
        let deserialized: ModulePermission = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "read_users");
        assert_eq!(deserialized.action, "read");
    }
}

mod api_config_tests {
    use super::*;

    #[test]
    fn api_config_serde_roundtrip() {
        let config = ApiConfig {
            enabled: true,
            path_prefix: Some("/v2".to_string()),
            openapi_path: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ApiConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.path_prefix, Some("/v2".to_string()));
        assert!(deserialized.openapi_path.is_none());
    }

    #[test]
    fn api_config_deserialize_minimal() {
        let json = r#"{"enabled": false}"#;
        let config: ApiConfig = serde_json::from_str(json).unwrap();
        assert!(!config.enabled);
        assert!(config.path_prefix.is_none());
    }
}
