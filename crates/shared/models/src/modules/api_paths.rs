//! API path constants.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AgentName, ClientId, ContentHash, PluginId, TenantId};

#[derive(Debug, Clone, Copy)]
pub struct ApiPaths;

impl ApiPaths {
    pub const GATEWAY_BASE: &'static str = "/v1";
    pub const GATEWAY_PUBLIC_BASE: &'static str = "/api/public/gateway";

    pub const API_BASE: &'static str = "/api";
    pub const API_V1: &'static str = "/api/v1";
    pub const CORE_BASE: &'static str = "/api/v1/core";
    pub const AGENTS_BASE: &'static str = "/api/v1/agents";
    pub const MCP_BASE: &'static str = "/api/v1/mcp";
    pub const STREAM_BASE: &'static str = "/api/v1/stream";
    pub const CONTENT_BASE: &'static str = "/api/v1/content";
    pub const SYNC_BASE: &'static str = "/api/v1/sync";
    pub const ANALYTICS_BASE: &'static str = "/api/v1/analytics";
    pub const META_BASE: &'static str = "/api/v1/meta";
    pub const SLACK_BASE: &'static str = "/api/v1/slack";
    pub const TEAMS_BASE: &'static str = "/api/v1/teams";

    pub const CORE_CONTEXTS: &'static str = "/api/v1/core/contexts";
    pub const CORE_TASKS: &'static str = "/api/v1/core/tasks";
    pub const CORE_ARTIFACTS: &'static str = "/api/v1/core/artifacts";

    pub const AGENT_RUNS: &'static str = "/api/v1/agent-runs";

    pub const AGENTS_REGISTRY: &'static str = "/api/v1/agents/registry";

    pub const MCP_REGISTRY: &'static str = "/api/v1/mcp/registry";

    pub const STREAM_CONTEXTS: &'static str = "/api/v1/stream/contexts";

    pub const AUTH_ME: &'static str = "/api/v1/auth/me";

    pub const OAUTH_BASE: &'static str = "/api/v1/core/oauth";
    pub const OAUTH_AUTHORIZE: &'static str = "/api/v1/core/oauth/authorize";
    pub const OAUTH_TOKEN: &'static str = "/api/v1/core/oauth/token";
    pub const OAUTH_CLIENTS: &'static str = "/api/v1/core/oauth/clients";

    pub const CORE_USERS: &'static str = "/api/v1/core/users";

    pub const WEBHOOK: &'static str = "/api/v1/webhook";

    pub const HEALTH: &'static str = "/api/v1/health";

    pub const LIVEZ: &'static str = "/livez";

    pub const READYZ: &'static str = "/readyz";
    pub const DISCOVERY: &'static str = "/api/v1";

    pub const WELLKNOWN_BASE: &'static str = "/.well-known";
    pub const WELLKNOWN_AGENT_CARD: &'static str = "/.well-known/agent-card.json";
    pub const WELLKNOWN_AGENT_CARDS: &'static str = "/.well-known/agent-cards";
    pub const WELLKNOWN_OAUTH_SERVER: &'static str = "/.well-known/oauth-authorization-server";
    pub const WELLKNOWN_OPENID_CONFIG: &'static str = "/.well-known/openid-configuration";
    pub const WELLKNOWN_OAUTH_PROTECTED: &'static str = "/.well-known/oauth-protected-resource";
    pub const WELLKNOWN_JWKS: &'static str = "/.well-known/jwks.json";

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

    pub const TRACK_BASE: &'static str = "/track";
    pub const TRACK_ENGAGEMENT: &'static str = "/track/engagement";

    pub const ADMIN_BASE: &'static str = "/api/v1/admin";

    pub const MARKETPLACE_BASE: &'static str = "/api/v1/marketplace";

    pub const CLOUD_TENANTS: &'static str = "/api/v1/tenants";

    pub const BRIDGE_BASE: &'static str = "/v1/bridge";
    pub const BRIDGE_PUBKEY: &'static str = "/v1/bridge/pubkey";
    pub const BRIDGE_MANIFEST: &'static str = "/v1/bridge/manifest";
    pub const BRIDGE_PLUGINS: &'static str = "/v1/bridge/plugins";
    pub const BRIDGE_PLUGIN_FILE_ROUTE: &'static str = "/v1/bridge/plugins/{plugin_id}/{*path}";
    pub const BRIDGE_LATEST: &'static str = "/v1/bridge/latest";
    pub const BRIDGE_DOWNLOAD: &'static str = "/v1/bridge/download";
    pub const BRIDGE_DOWNLOAD_ROUTE: &'static str = "/v1/bridge/download/{platform}";
    pub const BRIDGE_WHOAMI: &'static str = "/v1/bridge/whoami";
    pub const BRIDGE_PROFILE: &'static str = "/v1/bridge/profile";
    pub const BRIDGE_PROFILE_ENABLED_HOSTS: &'static str = "/v1/bridge/profile/enabled_hosts";
    pub const BRIDGE_PROFILE_HOST_MODEL_FILTER: &'static str =
        "/v1/bridge/profile/host-model-filter";
    pub const BRIDGE_PROFILE_USAGE: &'static str = "/v1/bridge/profile/usage";
    pub const BRIDGE_DEVICE: &'static str = "/v1/bridge/device";
    pub const BRIDGE_HEARTBEAT: &'static str = "/v1/bridge/heartbeat";
    pub const BRIDGE_STREAM: &'static str = "/v1/bridge/stream";
    pub const BRIDGE_NOTIFICATIONS: &'static str = "/v1/bridge/notifications";
    pub const BRIDGE_NOTIFICATIONS_READ: &'static str = "/v1/bridge/notifications/read";
    pub const BRIDGE_NOTIFICATIONS_SEEN: &'static str = "/v1/bridge/notifications/seen";
    pub const BRIDGE_NOTIFICATIONS_ESCALATE_ROUTE: &'static str =
        "/v1/bridge/notifications/{id}/escalate";
    pub const BRIDGE_NOTIFICATIONS_SUBSCRIPTIONS: &'static str =
        "/v1/bridge/notifications/subscriptions";
    pub const BRIDGE_NOTIFICATIONS_SUBSCRIPTION_ROUTE: &'static str =
        "/v1/bridge/notifications/subscriptions/{id}";
    pub const BRIDGE_NOTIFICATIONS_STREAMS: &'static str = "/v1/bridge/notifications/streams";
    pub const BRIDGE_DRIVE_MANIFEST: &'static str = "/v1/bridge/drive/manifest";
    pub const BRIDGE_DRIVE_CHANGES: &'static str = "/v1/bridge/drive/changes";
    pub const BRIDGE_DRIVE_BLOB: &'static str = "/v1/bridge/drive/blob";
    pub const BRIDGE_DRIVE_BLOB_ROUTE: &'static str = "/v1/bridge/drive/blob/{sha256}";
    pub const BRIDGE_DRIVE_COMMIT: &'static str = "/v1/bridge/drive/commit";
    pub const BRIDGE_DRIVE_PRESENCE: &'static str = "/v1/bridge/drive/presence";
    pub const BRIDGE_DRIVE_PROPOSALS: &'static str = "/v1/bridge/drive/proposals";
    pub const BRIDGE_DRIVE_TOKEN: &'static str = "/v1/bridge/drive/token";
    pub const DOCUMENTS: &'static str = "/api/v1/documents";
    pub const DOCUMENT_FOLDERS: &'static str = "/api/v1/document-folders";
    pub const UPLOADS: &'static str = "/api/v1/uploads";
    pub const NOTIFICATIONS: &'static str = "/api/v1/notifications";
    pub const NOTIFICATIONS_SEEN: &'static str = "/api/v1/notifications/seen";
    pub const ADMIN_NOTIFICATIONS: &'static str = "/api/v1/admin/notifications";
    pub const BRIDGE_AUTH_PAT: &'static str = "/v1/auth/bridge/pat";
    pub const BRIDGE_AUTH_SESSION: &'static str = "/v1/auth/bridge/session";
    pub const BRIDGE_AUTH_SESSION_PAT: &'static str = "/v1/auth/bridge/session-pat";
    pub const BRIDGE_AUTH_OAUTH_CLIENT: &'static str = "/v1/auth/bridge/oauth-client";
    pub const BRIDGE_AUTH_CAPABILITIES: &'static str = "/v1/auth/bridge/capabilities";
    pub const BRIDGE_PERSONAL_KEYS: &'static str = "/v1/me/provider-credentials";
    pub const OTEL: &'static str = "/otel";
    pub const OTEL_REST_ROUTE: &'static str = "/otel/{*rest}";
    pub const MCP: &'static str = "/mcp";
    pub const PUBLIC_HOOKS: &'static str = "/api/public/hooks";
    pub const PUBLIC_HOOKS_TRACK: &'static str = "/api/public/hooks/track";
    pub const CONSUMER_RECEIPTS: &'static str = "/api/v1/consumer/receipts";
    pub const CONSUMER_SESSION_BINDINGS: &'static str = "/api/v1/consumer/session-bindings";
    pub const CONSUMER_DEVICE_ENROLLMENT: &'static str = "/api/v1/consumer-devices/enrollment";
    pub const CONSUMER_BUNDLE_ROUTE: &'static str =
        "/api/v1/consumer/resources/{resource}/publications/{publication}/bundle";

    // Why: `/otel` and `/mcp` are route families, not single endpoints; the
    // bare path and every path below it belong to the family and nothing that
    // merely shares the prefix (`/otelx`, `/mcpx`) does. Stated once here so
    // the server's routing and the bridge's credential scoping cannot disagree.
    #[must_use]
    pub fn is_otel(path: &str) -> bool {
        Self::in_family(path, Self::OTEL)
    }

    #[must_use]
    pub fn is_mcp(path: &str) -> bool {
        Self::in_family(path, Self::MCP)
    }

    #[must_use]
    pub fn is_public_hook(path: &str) -> bool {
        path.len() > Self::PUBLIC_HOOKS.len() + 1 && Self::in_family(path, Self::PUBLIC_HOOKS)
    }

    fn in_family(path: &str, base: &str) -> bool {
        path == base
            || path
                .strip_prefix(base)
                .is_some_and(|rest| rest.starts_with('/'))
    }

    #[must_use]
    pub fn mcp_server_slug(path: &str) -> Option<&str> {
        let rest = path.strip_prefix(Self::MCP)?.strip_prefix('/')?;
        let slug = rest.split('/').next()?;
        (!slug.is_empty()).then_some(slug)
    }

    #[must_use]
    pub fn gateway_relative(path: &'static str) -> &'static str {
        path.strip_prefix(Self::GATEWAY_BASE).unwrap_or(path)
    }

    #[must_use]
    pub fn api_v1_relative(path: &'static str) -> &'static str {
        path.strip_prefix(Self::API_V1).unwrap_or(path)
    }

    pub fn bridge_plugin_file(plugin_id: &PluginId, relative_path: &str) -> String {
        format!(
            "{}/{}/{relative_path}",
            Self::BRIDGE_PLUGINS,
            plugin_id.as_str()
        )
    }

    pub fn bridge_drive_blob(sha256: &ContentHash) -> String {
        format!("{}/{}", Self::BRIDGE_DRIVE_BLOB, sha256.as_str())
    }

    pub fn bridge_plugin_archive(plugin_id: &PluginId, sha256: &str) -> String {
        format!(
            "{}/{}/archive/{sha256}",
            Self::BRIDGE_PLUGINS,
            plugin_id.as_str()
        )
    }

    pub fn bridge_download(platform: &str) -> String {
        format!("{}/{platform}", Self::BRIDGE_DOWNLOAD)
    }

    pub fn otel_under(base_url: &str) -> String {
        format!("{}{}", base_url.trim_end_matches('/'), Self::OTEL)
    }

    pub fn tenant(tenant_id: &TenantId) -> String {
        format!("{}/{}", Self::CLOUD_TENANTS, tenant_id.as_str())
    }

    pub fn tenant_status(tenant_id: &TenantId) -> String {
        format!("{}/{}/status", Self::CLOUD_TENANTS, tenant_id.as_str())
    }

    pub fn tenant_registry_token(tenant_id: &TenantId) -> String {
        format!(
            "{}/{}/registry-token",
            Self::CLOUD_TENANTS,
            tenant_id.as_str()
        )
    }

    pub fn tenant_deploy(tenant_id: &TenantId) -> String {
        format!("{}/{}/deploy", Self::CLOUD_TENANTS, tenant_id.as_str())
    }

    pub fn tenant_secrets(tenant_id: &TenantId) -> String {
        format!("{}/{}/secrets", Self::CLOUD_TENANTS, tenant_id.as_str())
    }

    pub fn tenant_rotate_credentials(tenant_id: &TenantId) -> String {
        format!(
            "{}/{}/rotate-credentials",
            Self::CLOUD_TENANTS,
            tenant_id.as_str()
        )
    }

    pub fn mcp_server_endpoint(server_name: &str) -> String {
        format!("{}/{}/mcp", Self::MCP_BASE, server_name)
    }

    pub fn oauth_client_location(client_id: &ClientId) -> String {
        format!("{}/{}", Self::OAUTH_CLIENTS, client_id.as_str())
    }

    pub fn wellknown_agent_card_named(agent_name: &str) -> String {
        format!("{}/{}", Self::WELLKNOWN_AGENT_CARDS, agent_name)
    }

    pub fn agent_endpoint(agent_name: &AgentName) -> String {
        format!("{}/{}/", Self::AGENTS_BASE, agent_name.as_str())
    }
}
