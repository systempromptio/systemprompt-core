use systemprompt_identifiers::{AgentId, ClientId, TenantId};
use systemprompt_models::modules::{ApiPaths, ServiceCategory};

#[test]
fn api_paths_constants_start_with_slash() {
    assert!(ApiPaths::API_BASE.starts_with('/'));
    assert!(ApiPaths::API_V1.starts_with('/'));
    assert!(ApiPaths::CORE_BASE.starts_with('/'));
    assert!(ApiPaths::AGENTS_BASE.starts_with('/'));
    assert!(ApiPaths::MCP_BASE.starts_with('/'));
    assert!(ApiPaths::HEALTH.starts_with('/'));
}

#[test]
fn api_paths_tenant_builds_path() {
    let id = TenantId::new("t1");
    let path = ApiPaths::tenant(&id);
    assert!(path.ends_with("/t1"));
    assert!(path.contains(ApiPaths::CLOUD_TENANTS));
}

#[test]
fn api_paths_tenant_status() {
    let id = TenantId::new("t2");
    let path = ApiPaths::tenant_status(&id);
    assert!(path.ends_with("/status"));
}

#[test]
fn api_paths_tenant_deploy() {
    let id = TenantId::new("t3");
    let path = ApiPaths::tenant_deploy(&id);
    assert!(path.ends_with("/deploy"));
}

#[test]
fn api_paths_tenant_events() {
    let id = TenantId::new("t4");
    let path = ApiPaths::tenant_events(&id);
    assert!(path.ends_with("/events"));
}

#[test]
fn api_paths_tenant_restart() {
    let id = TenantId::new("t5");
    let path = ApiPaths::tenant_restart(&id);
    assert!(path.ends_with("/restart"));
}

#[test]
fn api_paths_tenant_secrets() {
    let id = TenantId::new("t6");
    let path = ApiPaths::tenant_secrets(&id);
    assert!(path.ends_with("/secrets"));
}

#[test]
fn api_paths_tenant_registry_token() {
    let id = TenantId::new("t7");
    let path = ApiPaths::tenant_registry_token(&id);
    assert!(path.ends_with("/registry-token"));
}

#[test]
fn api_paths_tenant_rotate_credentials() {
    let id = TenantId::new("t8");
    let path = ApiPaths::tenant_rotate_credentials(&id);
    assert!(path.ends_with("/rotate-credentials"));
}

#[test]
fn api_paths_tenant_subscription_cancel() {
    let id = TenantId::new("t9");
    let path = ApiPaths::tenant_subscription_cancel(&id);
    assert!(path.ends_with("/subscription/cancel"));
}

#[test]
fn api_paths_tenant_custom_domain() {
    let id = TenantId::new("t10");
    let path = ApiPaths::tenant_custom_domain(&id);
    assert!(path.ends_with("/custom-domain"));
}

#[test]
fn api_paths_tenant_external_db_access() {
    let id = TenantId::new("t11");
    let path = ApiPaths::tenant_external_db_access(&id);
    assert!(path.ends_with("/external-db-access"));
}

#[test]
fn api_paths_mcp_server_endpoint() {
    let path = ApiPaths::mcp_server_endpoint("my_server");
    assert!(path.contains("my_server"));
    assert!(path.ends_with("/mcp"));
}

#[test]
fn api_paths_oauth_client_location() {
    let id = ClientId::new("client1");
    let path = ApiPaths::oauth_client_location(&id);
    assert!(path.contains("client1"));
}

#[test]
fn api_paths_wellknown_agent_card_named() {
    let path = ApiPaths::wellknown_agent_card_named("my_agent");
    assert!(path.contains("my_agent"));
    assert!(path.starts_with(ApiPaths::WELLKNOWN_AGENT_CARDS));
}

#[test]
fn api_paths_agent_endpoint() {
    let id = AgentId::new("agent1");
    let path = ApiPaths::agent_endpoint(&id);
    assert!(path.contains("agent1"));
}

#[test]
fn service_category_all_returns_four_variants() {
    assert_eq!(ServiceCategory::all().len(), 4);
}

#[test]
fn service_category_base_path_not_empty() {
    for cat in ServiceCategory::all() {
        let expected = match cat {
            ServiceCategory::Core => ApiPaths::CORE_BASE,
            ServiceCategory::Agent => ApiPaths::AGENTS_BASE,
            ServiceCategory::Mcp => ApiPaths::MCP_BASE,
            ServiceCategory::Meta => "/",
        };
        assert_eq!(cat.base_path(), expected, "{cat:?} base_path");
    }
}

#[test]
fn service_category_display_name_not_empty() {
    for cat in ServiceCategory::all() {
        let expected = match cat {
            ServiceCategory::Core => "Core",
            ServiceCategory::Agent => "Agent",
            ServiceCategory::Mcp => "MCP",
            ServiceCategory::Meta => "Meta",
        };
        assert_eq!(cat.display_name(), expected);
    }
}

#[test]
fn service_category_mount_path_empty_module() {
    let cat = ServiceCategory::Core;
    assert_eq!(cat.mount_path(""), ApiPaths::CORE_BASE);
}

#[test]
fn service_category_mount_path_non_empty_module() {
    let cat = ServiceCategory::Agent;
    let path = cat.mount_path("registry");
    assert!(path.contains("registry"));
    assert!(path.starts_with(ApiPaths::AGENTS_BASE));
}

#[test]
fn service_category_meta_mount_path_slash_prefix() {
    let cat = ServiceCategory::Meta;
    assert_eq!(cat.mount_path(""), "/");
    let path = cat.mount_path("foo");
    assert_eq!(path, "/foo");
}

#[test]
fn service_category_matches_path_core() {
    let cat = ServiceCategory::Core;
    assert!(cat.matches_path(ApiPaths::CORE_BASE));
    assert!(cat.matches_path("/api/v1/core/users"));
    assert!(!cat.matches_path("/api/v1/agents"));
}

#[test]
fn service_category_matches_path_agent() {
    let cat = ServiceCategory::Agent;
    assert!(cat.matches_path(ApiPaths::AGENTS_BASE));
    assert!(!cat.matches_path(ApiPaths::MCP_BASE));
}

#[test]
fn service_category_from_path_core() {
    assert_eq!(
        ServiceCategory::from_path("/api/v1/core/tasks"),
        Some(ServiceCategory::Core)
    );
}

#[test]
fn service_category_from_path_agent() {
    assert_eq!(
        ServiceCategory::from_path("/api/v1/agents/registry"),
        Some(ServiceCategory::Agent)
    );
}

#[test]
fn service_category_from_path_mcp() {
    assert_eq!(
        ServiceCategory::from_path("/api/v1/mcp/registry"),
        Some(ServiceCategory::Mcp)
    );
}

#[test]
fn service_category_from_path_unknown_returns_none() {
    assert_eq!(ServiceCategory::from_path("/api/v2/something"), None);
}

#[test]
fn service_category_serde_round_trip() {
    for cat in ServiceCategory::all() {
        let json = serde_json::to_string(cat).unwrap();
        let decoded: ServiceCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(&decoded, cat);
    }
}
