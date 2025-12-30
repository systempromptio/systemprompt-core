//! API path constants.

#[derive(Debug, Clone, Copy)]
pub struct ApiPaths;

impl ApiPaths {
    pub const API_BASE: &'static str = "/api";
    pub const API_V1: &'static str = "/api/v1";
    pub const CORE_BASE: &'static str = "/api/v1/core";
    pub const AGENTS_BASE: &'static str = "/api/v1/agents";
    pub const MCP_BASE: &'static str = "/api/v1/mcp";
    pub const STREAM_BASE: &'static str = "/api/v1/stream";
    pub const CONTENT_BASE: &'static str = "/api/v1/content";
    pub const META_BASE: &'static str = "/api/v1/meta";

    pub const CORE_CONTEXTS: &'static str = "/api/v1/core/contexts";
    pub const CORE_TASKS: &'static str = "/api/v1/core/tasks";
    pub const CORE_ARTIFACTS: &'static str = "/api/v1/core/artifacts";
    pub const CONTEXTS_WEBHOOK: &'static str = "/api/v1/core/contexts/webhook";

    pub const AGENTS_REGISTRY: &'static str = "/api/v1/agents/registry";

    pub const MCP_REGISTRY: &'static str = "/api/v1/mcp/registry";

    pub const STREAM_CONTEXTS: &'static str = "/api/v1/stream/contexts";
    pub const STREAM_AGUI: &'static str = "/api/v1/stream/agui";
    pub const STREAM_A2A: &'static str = "/api/v1/stream/a2a";

    pub const AUTH_ME: &'static str = "/api/v1/auth/me";

    pub const OAUTH_BASE: &'static str = "/api/v1/core/oauth";
    pub const OAUTH_SESSION: &'static str = "/api/v1/core/oauth/session";
    pub const OAUTH_REGISTER: &'static str = "/api/v1/core/oauth/register";
    pub const OAUTH_AUTHORIZE: &'static str = "/api/v1/core/oauth/authorize";
    pub const OAUTH_TOKEN: &'static str = "/api/v1/core/oauth/token";
    pub const OAUTH_CALLBACK: &'static str = "/api/v1/core/oauth/callback";
    pub const OAUTH_CONSENT: &'static str = "/api/v1/core/oauth/consent";
    pub const OAUTH_WEBAUTHN_COMPLETE: &'static str = "/api/v1/core/oauth/webauthn/complete";
    pub const OAUTH_CLIENTS: &'static str = "/api/v1/core/oauth/clients";

    pub const WEBHOOK: &'static str = "/api/v1/webhook";
    pub const WEBHOOK_AGUI: &'static str = "/api/v1/webhook/agui";
    pub const WEBHOOK_A2A: &'static str = "/api/v1/webhook/a2a";

    pub const HEALTH: &'static str = "/api/v1/health";
    pub const DISCOVERY: &'static str = "/api/v1";

    pub const WELLKNOWN_BASE: &'static str = "/.well-known";
    pub const WELLKNOWN_AGENT_CARD: &'static str = "/.well-known/agent-card.json";
    pub const WELLKNOWN_AGENT_CARDS: &'static str = "/.well-known/agent-cards";
    pub const WELLKNOWN_OAUTH_SERVER: &'static str = "/.well-known/oauth-authorization-server";
    pub const WELLKNOWN_OPENID_CONFIG: &'static str = "/.well-known/openid-configuration";
    pub const WELLKNOWN_OAUTH_PROTECTED: &'static str = "/.well-known/oauth-protected-resource";

    pub const A2A_CARD: &'static str = "/api/a2a/card";

    pub const ASSETS_BASE: &'static str = "/assets";
    pub const FILES_BASE: &'static str = "/files";
    pub const GENERATED_BASE: &'static str = "/generated";
    pub const IMAGES_BASE: &'static str = "/images";
    pub const STATIC_BASE: &'static str = "/static";
    pub const NEXT_BASE: &'static str = "/_next";
    pub const DOCS_BASE: &'static str = "/docs";
    pub const SWAGGER_BASE: &'static str = "/swagger";
    pub const OPENAPI_BASE: &'static str = "/openapi";

    pub const ADMIN_BASE: &'static str = "/api/v1/admin";
    pub const ADMIN_LOGS: &'static str = "/api/v1/admin/logs";
    pub const ADMIN_USERS: &'static str = "/api/v1/admin/users";
    pub const ADMIN_ANALYTICS: &'static str = "/api/v1/admin/analytics";
    pub const ADMIN_SESSIONS: &'static str = "/api/v1/admin/sessions";

    pub const CLOUD_TENANTS: &'static str = "/api/v1/tenants";
    pub const CLOUD_CHECKOUT: &'static str = "/api/v1/checkout";
    pub const CLOUD_CHECKOUT_PLANS: &'static str = "/api/v1/checkout/plans";

    pub fn tenant(tenant_id: &str) -> String {
        format!("{}/{}", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_status(tenant_id: &str) -> String {
        format!("{}/{}/status", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_registry_token(tenant_id: &str) -> String {
        format!("{}/{}/registry-token", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_deploy(tenant_id: &str) -> String {
        format!("{}/{}/deploy", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_events(tenant_id: &str) -> String {
        format!("{}/{}/events", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_logs(tenant_id: &str) -> String {
        format!("{}/{}/logs", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_restart(tenant_id: &str) -> String {
        format!("{}/{}/restart", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_retry_provision(tenant_id: &str) -> String {
        format!("{}/{}/retry-provision", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn tenant_secrets(tenant_id: &str) -> String {
        format!("{}/{}/secrets", Self::CLOUD_TENANTS, tenant_id)
    }

    pub fn mcp_server_endpoint(server_name: &str) -> String {
        format!("{}/{}/mcp", Self::MCP_BASE, server_name)
    }

    pub fn oauth_client_location(client_id: &str) -> String {
        format!("{}/{}", Self::OAUTH_CLIENTS, client_id)
    }

    pub fn wellknown_agent_card_named(agent_name: &str) -> String {
        format!("{}/{}", Self::WELLKNOWN_AGENT_CARDS, agent_name)
    }

    pub fn agent_endpoint(agent_id: &str) -> String {
        format!("{}/{}/", Self::AGENTS_BASE, agent_id)
    }
}
