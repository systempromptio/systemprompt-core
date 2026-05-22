# HTTP API Reference

The complete HTTP surface of the systemprompt-core API server, grouped by area. Each route lists its method, path, authentication requirement, and purpose.

A2A is the agent-to-agent protocol; MCP is the Model Context Protocol. The gateway is the provider-facing proxy that accepts vendor-shaped requests (Anthropic Messages, OpenAI Responses) and routes them through the platform.

## Conventions

- Path constants are defined in `crates/shared/models/src/modules/api_paths.rs`. Routes are mounted in `crates/entry/api/src/services/server/discovery.rs`, `crates/entry/api/src/services/server/routes/protocol.rs`, and the routers under `crates/entry/api/src/routes/`.
- All paths in this document are relative to the configured external base URL (`api_external_url` in the profile). Examples below use `http://127.0.0.1:8080`.
- `{name}` denotes a path parameter.

### Authentication

The token plane is JWT, RS256 only. Alg-confusion and `none` are rejected; `kid` is required; `exp`/`nbf`/`iat` are validated with 30 s leeway; the `act` delegation chain is depth-capped (`crates/infra/security/src/auth/validation.rs`). There is no ES256/ES384/EdDSA verification path. Audience validation (`validate_aud`) is currently disabled — do not rely on audience isolation.

Authorization runs through a fail-closed authorization hook (default deny). Each route group is mounted with one of these policies:

| Policy in this doc | Meaning |
|--------------------|---------|
| Public | No bearer token required. |
| Authenticated | Any valid JWT. |
| User | User-type JWT (`AuthzPolicy::user()`). |
| Admin | Admin-type JWT. |
| Restricted | A named set of user types (stated per route). |

Tokens are supplied as `Authorization: Bearer <jwt>`.

### Request limits and middleware

- Body limit: 2 MiB on every route (`DefaultBodyLimit::max(2 * 1024 * 1024)`, `crates/entry/api/src/services/server/builder.rs:93`).
- Rate limiting: per-route `tower_governor` limiters, configured under `rate_limits` in the profile. Limiting can be disabled in development (`rate_limits.disabled`).
- A trailing-slash normaliser, CORS, security headers, IP-ban middleware, and JTI revocation middleware are applied globally (`builder.rs`, `routes.rs`).

### Error response shape

Errors serialize as a JSON `ApiError` object (`crates/shared/models/src/api/errors/mod.rs`):

```json
{
  "code": "not_found",
  "message": "Execution not found: exec_123",
  "details": null,
  "error_key": null,
  "path": null,
  "validation_errors": [],
  "timestamp": "2026-05-22T12:00:00Z",
  "trace_id": null
}
```

`code` is one of: `not_found`, `bad_request`, `unauthorized`, `forbidden`, `internal_error`, `validation_error`, `conflict_error`, `rate_limited`, `service_unavailable`. Fields that are absent are omitted from the payload. Each `validation_errors` entry carries `field`, `message`, `code`, and optional `context`.

## Discovery

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/api/v1` | Public | Root service discovery (links to health, oauth, core, agents, mcp, stream, well-known). |
| GET | `/api/v1/core` | Public | Core-services discovery document. |
| GET | `/api/v1/agents` | Public | Agent-services discovery document. |
| GET | `/api/v1/mcp` | Public | MCP-services discovery document. |

```bash
curl http://127.0.0.1:8080/api/v1
```

## Health and metrics

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/health` | Public | Liveness. |
| GET | `/api/v1/health` | Public | Liveness. |
| GET | `/api/v1/core/oauth/health` | Public | OAuth subsystem liveness. |
| GET | `/api/v1/health/detail` | Authenticated | Detailed health (subsystem status). |
| GET | `/metrics` | Public | Prometheus exposition. Always mounted; restrict it at the network or proxy layer. |

There are no `/health/live` or `/health/ready` aliases.

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/metrics
```

## OAuth / OIDC

Base path `/api/v1/core/oauth` (`crates/entry/api/src/routes/oauth/core.rs`). OAuth 2.x with OIDC and WebAuthn; PKCE method S256.

### Public OAuth endpoints

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/api/v1/core/oauth/health` | Public | Subsystem health. |
| POST | `/api/v1/core/oauth/session` | Public | Issue an anonymous session token. |
| GET | `/api/v1/core/oauth/authorize` | Public | Authorization endpoint (browser flow). |
| POST | `/api/v1/core/oauth/authorize` | Public | Authorization decision submission. |
| GET | `/api/v1/core/oauth/callback` | Public | Upstream IdP callback. |
| POST | `/api/v1/core/oauth/token` | Public | Token endpoint (authorization_code, refresh_token, client_credentials). |
| GET | `/api/v1/core/oauth/webauthn/complete` | Public | Complete a WebAuthn-bound authorization. |
| POST | `/api/v1/core/oauth/webauthn/register/start` | Public | Begin WebAuthn credential registration. |
| POST | `/api/v1/core/oauth/webauthn/register/finish` | Public | Finish WebAuthn credential registration. |
| POST | `/api/v1/core/oauth/webauthn/auth/start` | Public | Begin WebAuthn authentication. |
| POST | `/api/v1/core/oauth/webauthn/auth/finish` | Public | Finish WebAuthn authentication. |
| GET | `/api/v1/core/oauth/webauthn/link/start` | Public | Begin linking a passkey to an account. |
| POST | `/api/v1/core/oauth/webauthn/link/finish` | Public | Finish linking a passkey. |

### Authenticated OAuth endpoints

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| POST | `/api/v1/core/oauth/introspect` | User | RFC 7662 token introspection. |
| POST | `/api/v1/core/oauth/revoke` | User | RFC 7009 token revocation. |
| POST | `/api/v1/core/oauth/logout` | User | End the current session. |
| GET | `/api/v1/core/oauth/userinfo` | User | OIDC userinfo. |
| GET | `/api/v1/core/oauth/consent` | User | Render consent state. |
| POST | `/api/v1/core/oauth/consent` | User | Submit consent decision. |
| POST | `/api/v1/core/oauth/register` | User | Dynamic client registration (RFC 7591). |
| GET | `/api/v1/core/oauth/register/{client_id}` | User | Read a registered client. |
| PUT | `/api/v1/core/oauth/register/{client_id}` | User | Update a registered client. |
| DELETE | `/api/v1/core/oauth/register/{client_id}` | User | Delete a registered client. |
| * | `/api/v1/core/oauth/clients/*` | User | Client management subtree. |

### Well-known documents

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/.well-known/openid-configuration` | Public | OIDC discovery. |
| GET | `/.well-known/oauth-authorization-server` | Public | OAuth AS metadata. |
| GET | `/.well-known/oauth-protected-resource` | Public | Protected-resource metadata. |
| GET | `/.well-known/oauth-protected-resource/{path}` | Public | Per-resource protected-resource metadata. |
| GET | `/.well-known/jwks.json` | Public | JSON Web Key Set for the JWT plane. |
| GET | `/.well-known/agent-card.json` | Public | Default A2A agent card. |
| GET | `/.well-known/agent-cards` | Public | List A2A agent cards. |
| GET | `/.well-known/agent-cards/{agent_name}` | Public | A2A agent card for a named agent. |

```bash
curl http://127.0.0.1:8080/.well-known/openid-configuration
```

## Core: contexts, tasks, artifacts, users

### Contexts — base `/api/v1/core/contexts` (User)

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/v1/core/contexts` | List contexts with stats. |
| POST | `/api/v1/core/contexts` | Create a context. |
| GET | `/api/v1/core/contexts/{id}` | Get a context. |
| PUT | `/api/v1/core/contexts/{id}` | Update a context. |
| DELETE | `/api/v1/core/contexts/{id}` | Delete a context. |
| GET | `/api/v1/core/contexts/{context_id}/tasks` | List tasks in a context. |
| GET | `/api/v1/core/contexts/{context_id}/artifacts` | List artifacts in a context. |
| POST | `/api/v1/core/contexts/{context_id}/notifications` | Post a context notification. |
| POST | `/api/v1/core/contexts/{context_id}/events` | Forward an event into a context. |

### Tasks — base `/api/v1/core/tasks` (User)

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/v1/core/tasks` | List the caller's tasks. |
| GET | `/api/v1/core/tasks/{task_id}` | Get a task. |
| DELETE | `/api/v1/core/tasks/{task_id}` | Delete a task. |
| GET | `/api/v1/core/tasks/{task_id}/messages` | List messages for a task. |
| GET | `/api/v1/core/tasks/{task_id}/artifacts` | List artifacts produced by a task. |

### Artifacts — base `/api/v1/core/artifacts` (User)

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/v1/core/artifacts` | List the caller's artifacts. |
| GET | `/api/v1/core/artifacts/{artifact_id}` | Get an artifact. |
| GET | `/api/v1/core/artifacts/{artifact_id}/ui` | Get an artifact's rendered UI resource. |

### Users — base `/api/v1/core/users` (User)

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/v1/core/users/me/sessions/revoke_all` | Revoke all of the caller's sessions. |

### Context webhooks — base `/api/v1/webhook` (Authenticated)

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/v1/webhook/broadcast` | Broadcast a context event. |
| POST | `/api/v1/webhook/agui` | Broadcast an AG-UI event. |
| POST | `/api/v1/webhook/a2a` | Broadcast an A2A event. |

```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/api/v1/core/contexts
```

## Agents (A2A)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/api/v1/agents/registry` | Public | List registered agents and their cards. |
| * | `/api/v1/agents/{service_name}` | Authenticated | Proxy a request to a named agent (A2A JSON-RPC). |
| * | `/api/v1/agents/{service_name}/{path}` | Authenticated | Proxy to a sub-path of a named agent. |

The proxy routes accept any HTTP method (`crates/entry/api/src/routes/proxy/agents.rs`).

```bash
curl http://127.0.0.1:8080/api/v1/agents/registry
```

## MCP

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/api/v1/mcp/registry` | Public | List registered MCP servers. |
| GET | `/api/v1/mcp/executions/{id}` | Restricted | Fetch a recorded tool execution. |
| GET | `/api/v1/mcp/{service_name}/mcp/.well-known/oauth-protected-resource` | Restricted | Per-server protected-resource metadata. |
| GET | `/api/v1/mcp/{service_name}/mcp/.well-known/oauth-authorization-server` | Restricted | Per-server AS metadata. |
| * | `/api/v1/mcp/{service_name}/{path}` | Restricted | Proxy to a named MCP server. The streamable-http transport endpoint is `/api/v1/mcp/{server}/mcp`. |

The MCP subtree is restricted to user types `User`, `Admin`, `Mcp`, and `Service` (`protocol.rs:126`). MCP protocol version 1.6.

```bash
curl http://127.0.0.1:8080/api/v1/mcp/registry
```

## Streaming (Server-Sent Events)

Base `/api/v1/stream` (User). Each route opens an SSE stream with keep-alive; per-user connection caps apply and a saturated cap returns `429 Too Many Requests`.

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/v1/stream/contexts` | Stream context-state updates. |
| GET | `/api/v1/stream/agui` | Stream AG-UI events. |
| GET | `/api/v1/stream/a2a` | Stream A2A events. |

```bash
curl -N -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/api/v1/stream/contexts
```

## Gateway (provider-facing)

Base `/v1` (`crates/entry/api/src/routes/gateway/mod.rs`). The gateway mounts only when analytics and user providers are available. Authentication is resolved per request by the gateway handlers (bearer JWT, bridge PAT, session, or mTLS).

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/v1/` | Gateway root descriptor. |
| GET | `/v1/models` | List available models. |
| POST | `/v1/messages` | Anthropic Messages-shaped inbound request. |
| POST | `/v1/responses` | OpenAI Responses-shaped inbound request. |
| POST | `/v1/otel` | OTLP telemetry ingest. |
| POST | `/v1/otel/{rest}` | OTLP telemetry ingest (sub-path). |
| POST | `/v1/auth/bridge/pat` | Exchange a personal access token. |
| POST | `/v1/auth/bridge/session` | Exchange a session credential. |
| POST | `/v1/auth/bridge/mtls` | Exchange an mTLS client certificate. |
| POST | `/v1/auth/bridge/oauth-client` | Provision a bridge OAuth client. |
| GET | `/v1/auth/bridge/capabilities` | List supported bridge auth methods. |
| GET | `/v1/bridge/pubkey` | Server signing public key. |
| GET | `/v1/bridge/profile` | Bridge profile descriptor. |
| GET | `/v1/bridge/whoami` | Identify the authenticated bridge principal. |
| GET | `/v1/bridge/manifest` | Bridge manifest (agents, hooks, skills). |
| POST | `/v1/bridge/profile/enabled_hosts` | Set per-host enable state. |
| GET | `/v1/bridge/profile/usage` | Profile usage report. |
| POST | `/v1/bridge/heartbeat` | Bridge heartbeat. |

```bash
curl http://127.0.0.1:8080/v1/models
```

## Webhooks

See [Context webhooks](#context-webhooks--base-apiv1webhook-authenticated) above. Base `/api/v1/webhook`, authenticated, with `/broadcast`, `/agui`, and `/a2a`.

## Other mounted areas

These areas are mounted by `mount_content_and_misc` (`protocol.rs:149`).

| Base path | Auth | Purpose |
|-----------|------|---------|
| `/api/v1/content` | Public + User | Content query and link resolution. |
| `/api/v1/sync` | Restricted (Service) | Cloud sync of files. |
| `/api/v1/marketplace` | Public | Marketplace catalog. |
| `/api/v1/analytics` | Admin | Analytics events and streams. |
| `/track/engagement` | Public | Engagement tracking ingest. |
| `/api/v1/admin` | Admin | Admin operations (logs, users, CLI relay, keys). |
| `/auth/link-passkey` | Public | Passkey-linking page. |
