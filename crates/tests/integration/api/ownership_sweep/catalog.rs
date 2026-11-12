use super::table::{Row, free};

const PUBLIC: &str =
    "unauthenticated public metadata (health, discovery, well-known, JWKS); no per-user resource";
const ADMIN: &str =
    "admin-only operator surface, authorised by role rather than by resource ownership";
const OAUTH: &str = "OAuth/WebAuthn/SSO protocol endpoint; the presented credential or flow state is the subject, no stored user resource is addressed by id";
const REGISTER: &str = "RFC 7592 client configuration, authorised by the client's registration access token rather than a user";
const INFERENCE: &str =
    "AI gateway call acting on the caller's own request; no stored resource is addressed";
const BRIDGE_SELF: &str = "bridge self-service route acting only on the authenticated caller's own profile, device or session; no foreign id is addressable";
const CALLER: &str = "acts only on the authenticated caller's own identity or sessions; the path carries no resource id";
const WEBHOOK: &str =
    "inbound platform webhook authenticated by the platform's signature, not by a user";
const CONTENT: &str =
    "published content and link redirects/analytics, readable by design and not owned by a user";
const MCP_HOP: &str = "session-less MCP-server call authenticated by the proxy hop token; the subject comes from the token and its ownership of the target is re-checked before signing (internal_links tests)";
const CATALOG: &str = "org-wide catalog (marketplace, registry, schema, subject directory) identical for every caller";
const PROXY: &str = "service proxy authorised per service by the proxy access check; owned resources behind it are probed on their own routes";
const INGEST: &str =
    "write-only ingest of the caller's own analytics or telemetry; nothing is addressed by id";
const STREAM: &str =
    "server-sent event stream filtered to the caller's own events by the event router";
const SERVICE: &str = "service-to-service sync restricted to the Service user type";
const GOVERN: &str =
    "brain governance queue authorised by the steward role, not by resource ownership";
const PUBLISH: &str =
    "stream publish is admitted by the stream's declared publishers, not by ownership";
const BROADCAST: &str =
    "broadcasts only into the caller's own event stream; the body's user_id must equal the caller";
const CONSUMER: &str =
    "managed-consumer device protocol authenticated by the device enrollment credential";
const TOKEN: &str = "issues a Drive sync token for the calling device user only";
const STATIC: &str = "static site and prerendered page serving; no per-user resource";

#[rustfmt::skip]
pub(super) const NOT_USER_OWNED: &[Row] = &[
    free("routes/admin/cli.rs::router_with_binary", "POST", r#""/""#, ADMIN),
    free("routes/admin/keys.rs::router", "DELETE", r#""/{key_id}""#, ADMIN),
    free("routes/admin/keys.rs::router", "GET", r#""/""#, ADMIN),
    free("routes/admin/keys.rs::router", "POST", r#""/""#, ADMIN),
    free("routes/admin/services/mod.rs::router", "GET", r#""/status""#, ADMIN),
    free("routes/admin/services/mod.rs::router", "POST", r#""/refresh""#, ADMIN),
    free("routes/agent/contexts/mod.rs::webhook_router", "POST", r#""/a2a""#, BROADCAST),
    free("routes/agent/contexts/mod.rs::webhook_router", "POST", r#""/agui""#, BROADCAST),
    free("routes/agent/registry.rs::router", "GET", r#""/""#, CATALOG),
    free("routes/analytics/mod.rs::routes", "GET", r#""/stream""#, INGEST),
    free("routes/analytics/mod.rs::routes", "POST", r#""/events""#, INGEST),
    free("routes/analytics/mod.rs::routes", "POST", r#""/events/batch""#, INGEST),
    free("routes/brain/mod.rs::admin_router", "POST", r#""/simulate""#, ADMIN),
    free("routes/brain/mod.rs::router", "GET", r#""/proposals""#, GOVERN),
    free("routes/brain/mod.rs::router", "POST", r#""/proposals/{id}/approve""#, GOVERN),
    free("routes/brain/mod.rs::router", "POST", r#""/proposals/{id}/reject""#, GOVERN),
    free("routes/brain/mod.rs::router", "POST", r#""/proposals/{id}/replay""#, GOVERN),
    free("routes/content/mod.rs::authenticated_router", "POST", r#""/links/generate""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#""/links""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#""/links/campaigns/{campaign_id}/performance""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#""/links/journey""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#""/links/{link_id}/clicks""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#""/links/{link_id}/performance""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#""/{source_id}""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#""/{source_id}/{slug}""#, CONTENT),
    free("routes/content/mod.rs::public_router", "GET", r#"&format!("/{{source_id}}/{{slug}}/{}",suffix)"#, CONTENT),
    free("routes/content/mod.rs::public_router", "POST", r#""/query""#, CONTENT),
    free("routes/content/mod.rs::redirect_router", "GET", r#""/r/{short_code}""#, CONTENT),
    free("routes/documents/drive.rs::router", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_TOKEN)", TOKEN),
    free("routes/documents/mod.rs::router", "GET", r#""/schema""#, CATALOG),
    free("routes/documents/mod.rs::router", "GET", r#""/subject-kinds""#, CATALOG),
    free("routes/documents/mod.rs::router", "GET", r#""/subjects""#, CATALOG),
    free("routes/engagement/mod.rs::routes", "POST", r#""/""#, INGEST),
    free("routes/engagement/mod.rs::routes", "POST", r#""/batch""#, INGEST),
    free("routes/gateway/bridge_release/mod.rs::routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DOWNLOAD_ROUTE)", PUBLIC),
    free("routes/gateway/bridge_release/mod.rs::routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_LATEST)", PUBLIC),
    free("routes/gateway/mod.rs::gateway_parts", "GET", r#""/""#, CATALOG),
    free("routes/gateway/mod.rs::gateway_parts", "GET", r#""/models""#, CATALOG),
    free("routes/gateway/routers.rs::bridge_auth_routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_CAPABILITIES)", OAUTH),
    free("routes/gateway/routers.rs::bridge_auth_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT)", OAUTH),
    free("routes/gateway/routers.rs::bridge_auth_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_PAT)", OAUTH),
    free("routes/gateway/routers.rs::bridge_auth_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_SESSION)", OAUTH),
    free("routes/gateway/routers.rs::bridge_auth_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_SESSION_PAT)", OAUTH),
    free("routes/gateway/routers.rs::bridge_profile_routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_MANIFEST)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_profile_routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_profile_routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_PUBKEY)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_profile_routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_WHOAMI)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_profile_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DEVICE)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_profile_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_ENABLED_HOSTS)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_profile_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_HOST_MODEL_FILTER)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_session_routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_USAGE)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_session_routes", "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_STREAM)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::bridge_session_routes", "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_HEARTBEAT)", BRIDGE_SELF),
    free("routes/gateway/routers.rs::inference_routes", "POST", r#""/chat/completions""#, INFERENCE),
    free("routes/gateway/routers.rs::inference_routes", "POST", r#""/messages""#, INFERENCE),
    free("routes/gateway/routers.rs::inference_routes", "POST", r#""/messages/count_tokens""#, INFERENCE),
    free("routes/gateway/routers.rs::inference_routes", "POST", r#""/responses""#, INFERENCE),
    free("routes/gateway/routers.rs::otel_routes", "POST", "ApiPaths::OTEL", INGEST),
    free("routes/gateway/routers.rs::otel_routes", "POST", "ApiPaths::OTEL_REST_ROUTE", INGEST),
    free("routes/gateway/sessions.rs::public_router", "POST", r#""/sessions""#, BRIDGE_SELF),
    free("routes/internal/mod.rs::links_router", "POST", r#""/""#, MCP_HOP),
    free("routes/issues/mod.rs::router", "GET", r#""/admin""#, ADMIN),
    free("routes/issues/mod.rs::router", "PATCH", r#""/admin/{id}""#, ADMIN),
    free("routes/managed/consumer/mod.rs::router", "GET", "ApiPaths::api_v1_relative(ApiPaths::CONSUMER_BUNDLE_ROUTE)", CONSUMER),
    free("routes/managed/consumer/mod.rs::router", "POST", "ApiPaths::api_v1_relative(ApiPaths::CONSUMER_DEVICE_ENROLLMENT)", CONSUMER),
    free("routes/managed/consumer/mod.rs::router", "POST", "ApiPaths::api_v1_relative(ApiPaths::CONSUMER_RECEIPTS)", CONSUMER),
    free("routes/managed/consumer/mod.rs::router", "POST", "ApiPaths::api_v1_relative(ApiPaths::CONSUMER_SESSION_BINDINGS)", CONSUMER),
    free("routes/marketplace.rs::router", "GET", r#""/marketplace.json""#, CATALOG),
    free("routes/marketplace.rs::router", "GET", r#""/marketplaces""#, CATALOG),
    free("routes/marketplace.rs::router", "GET", r#""/marketplaces/{id}""#, CATALOG),
    free("routes/marketplace.rs::router", "GET", r#""/marketplaces/{id}/manifest.yaml""#, CATALOG),
    free("routes/marketplace.rs::router", "GET", r#""/plugins/{plugin_id}/{*path}""#, CATALOG),
    free("routes/mcp/mod.rs::registry_router", "GET", r#""/""#, CATALOG),
    free("routes/notifications/mod.rs::admin_router", "POST", r#""/simulate""#, ADMIN),
    free("routes/notifications/mod.rs::router", "POST", r#""/streams/{id}/publish""#, PUBLISH),
    free("routes/oauth/core.rs::authenticated_router", "GET", r#""/consent""#, OAUTH),
    free("routes/oauth/core.rs::authenticated_router", "GET", r#""/userinfo""#, OAUTH),
    free("routes/oauth/core.rs::authenticated_router", "POST", r#""/consent""#, OAUTH),
    free("routes/oauth/core.rs::authenticated_router", "POST", r#""/introspect""#, OAUTH),
    free("routes/oauth/core.rs::authenticated_router", "POST", r#""/logout""#, OAUTH),
    free("routes/oauth/core.rs::authenticated_router", "POST", r#""/revoke""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "DELETE", r#""/register/{client_id}""#, REGISTER),
    free("routes/oauth/core.rs::public_router", "GET", r#""/authorize""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "GET", r#""/health""#, PUBLIC),
    free("routes/oauth/core.rs::public_router", "GET", r#""/register/{client_id}""#, REGISTER),
    free("routes/oauth/core.rs::public_router", "GET", r#""/webauthn/complete""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "GET", r#""/webauthn/link/start""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/authorize""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/register""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/session""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/token""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/webauthn/auth/finish""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/webauthn/auth/start""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/webauthn/link/finish""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/webauthn/register/finish""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "POST", r#""/webauthn/register/start""#, OAUTH),
    free("routes/oauth/core.rs::public_router", "PUT", r#""/register/{client_id}""#, REGISTER),
    free("routes/oauth/wellknown.rs::wellknown_routes", "GET", r#""/.well-known/oauth-protected-resource/{*path}""#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "GET", r#"&format!("{}/",ApiPaths::WELLKNOWN_OAUTH_PROTECTED)"#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "GET", r#"&format!("{}/",ApiPaths::WELLKNOWN_OAUTH_SERVER)"#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "GET", r#"&format!("{}/",ApiPaths::WELLKNOWN_OPENID_CONFIG)"#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "GET", "ApiPaths::WELLKNOWN_OAUTH_PROTECTED", PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "GET", "ApiPaths::WELLKNOWN_OAUTH_SERVER", PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "GET", "ApiPaths::WELLKNOWN_OPENID_CONFIG", PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "OPTIONS", r#""/.well-known/oauth-protected-resource/{*path}""#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "OPTIONS", r#"&format!("{}/",ApiPaths::WELLKNOWN_OAUTH_PROTECTED)"#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "OPTIONS", r#"&format!("{}/",ApiPaths::WELLKNOWN_OAUTH_SERVER)"#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "OPTIONS", r#"&format!("{}/",ApiPaths::WELLKNOWN_OPENID_CONFIG)"#, PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "OPTIONS", "ApiPaths::WELLKNOWN_OAUTH_PROTECTED", PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "OPTIONS", "ApiPaths::WELLKNOWN_OAUTH_SERVER", PUBLIC),
    free("routes/oauth/wellknown.rs::wellknown_routes", "OPTIONS", "ApiPaths::WELLKNOWN_OPENID_CONFIG", PUBLIC),
    free("routes/proxy/agents.rs::router", "ANY", r#""/{service_name}""#, PROXY),
    free("routes/proxy/agents.rs::router", "ANY", r#""/{service_name}/{*path}""#, PROXY),
    free("routes/proxy/mcp/mod.rs::router", "ANY", r#""/{service_name}/{*path}""#, PROXY),
    free("routes/proxy/mcp/mod.rs::router", "GET", r#""/{service_name}/mcp/.well-known/oauth-authorization-server""#, PUBLIC),
    free("routes/proxy/mcp/mod.rs::router", "GET", r#""/{service_name}/mcp/.well-known/oauth-protected-resource""#, PUBLIC),
    free("routes/slack/mod.rs::slack_router", "POST", r#""/commands""#, WEBHOOK),
    free("routes/slack/mod.rs::slack_router", "POST", r#""/events""#, WEBHOOK),
    free("routes/slack/mod.rs::slack_router", "POST", r#""/interactivity""#, WEBHOOK),
    free("routes/sso/mod.rs::router", "GET", r#""/{id}/callback""#, OAUTH),
    free("routes/sso/mod.rs::router", "GET", r#""/{id}/metadata""#, OAUTH),
    free("routes/sso/mod.rs::router", "GET", r#""/{id}/start""#, OAUTH),
    free("routes/sso/mod.rs::router", "POST", r#""/{id}/callback""#, OAUTH),
    free("routes/stream/mod.rs::stream_router", "GET", r#""/a2a""#, STREAM),
    free("routes/stream/mod.rs::stream_router", "GET", r#""/agui""#, STREAM),
    free("routes/stream/mod.rs::stream_router", "GET", r#""/contexts""#, STREAM),
    free("routes/sync/mod.rs::router", "GET", r#""/files""#, SERVICE),
    free("routes/sync/mod.rs::router", "GET", r#""/files/manifest""#, SERVICE),
    free("routes/teams/mod.rs::teams_router", "POST", r#""/messages""#, WEBHOOK),
    free("routes/users/mod.rs::router", "POST", r#""/me/sessions/revoke_all""#, CALLER),
    free("routes/wellknown/agent_cards.rs::wellknown_router", "GET", r#"&format!("{}/{{agent_name}}",ApiPaths::WELLKNOWN_AGENT_CARDS)"#, PUBLIC),
    free("routes/wellknown/agent_cards.rs::wellknown_router", "GET", "ApiPaths::WELLKNOWN_AGENT_CARD", PUBLIC),
    free("routes/wellknown/agent_cards.rs::wellknown_router", "GET", "ApiPaths::WELLKNOWN_AGENT_CARDS", PUBLIC),
    free("routes/wellknown/bridge.rs::bridge_discovery_router", "GET", "ApiPaths::WELLKNOWN_BRIDGE", PUBLIC),
    free("routes/wellknown/jwks.rs::jwks_router", "GET", "ApiPaths::WELLKNOWN_JWKS", PUBLIC),
    free("services/server/discovery.rs::authenticated_discovery_router", "GET", r#""/api/v1/health/detail""#, ADMIN),
    free("services/server/discovery.rs::discovery_router", "GET", r#""/health""#, PUBLIC),
    free("services/server/discovery.rs::discovery_router", "GET", "ApiPaths::AGENTS_BASE", PUBLIC),
    free("services/server/discovery.rs::discovery_router", "GET", "ApiPaths::CORE_BASE", PUBLIC),
    free("services/server/discovery.rs::discovery_router", "GET", "ApiPaths::DISCOVERY", PUBLIC),
    free("services/server/discovery.rs::discovery_router", "GET", "ApiPaths::HEALTH", PUBLIC),
    free("services/server/discovery.rs::discovery_router", "GET", "ApiPaths::LIVEZ", PUBLIC),
    free("services/server/discovery.rs::discovery_router", "GET", "ApiPaths::MCP_BASE", PUBLIC),
    free("services/server/discovery.rs::discovery_router", "GET", "ApiPaths::READYZ", PUBLIC),
    free("services/server/discovery.rs::probes_router", "GET", r#""/health""#, PUBLIC),
    free("services/server/discovery.rs::probes_router", "GET", "ApiPaths::HEALTH", PUBLIC),
    free("services/server/discovery.rs::probes_router", "GET", "ApiPaths::LIVEZ", PUBLIC),
    free("services/server/discovery.rs::probes_router", "GET", "ApiPaths::READYZ", PUBLIC),
    free("services/server/metrics.rs::metrics_router", "GET", r#""/metrics""#, ADMIN),
    free("services/server/routes/mod.rs::link_passkey_router", "GET", r#""/auth/link-passkey""#, OAUTH),
    free("services/server/routes/static_setup.rs::build_static_router", "GET", r#""/""#, STATIC),
    free("services/server/startup.rs::starting_router", "GET", r#""/health""#, PUBLIC),
    free("services/server/startup.rs::starting_router", "GET", "ApiPaths::HEALTH", PUBLIC),
    free("services/server/startup.rs::starting_router", "GET", "ApiPaths::LIVEZ", PUBLIC),
    free("services/server/startup.rs::starting_router", "GET", "ApiPaths::READYZ", PUBLIC),
];
