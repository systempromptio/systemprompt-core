# HTTP API Reference

The complete HTTP surface of the systemprompt-core API server, grouped by area. Each route lists its method, path, authentication requirement, and purpose.

A2A is the agent-to-agent protocol; MCP is the Model Context Protocol. The gateway is the provider-facing proxy that accepts vendor-shaped requests (Anthropic Messages, OpenAI Responses) and routes them through the platform.

## Conventions

- Path constants are defined in `crates/shared/models/src/modules/api_paths.rs`. Routes are mounted in `crates/entry/api/src/services/server/discovery.rs`, `crates/entry/api/src/services/server/routes/protocol.rs`, and the routers under `crates/entry/api/src/routes/`.
- All paths in this document are relative to the configured external base URL (`api_external_url` in the profile). Examples below use `http://127.0.0.1:8080`.
- `{name}` denotes a path parameter.

### Authentication

The token plane is JWT, RS256 only. Alg-confusion and `none` are rejected; `kid` is required; `exp`/`nbf` are validated with 30 s leeway; the `act` delegation chain is depth-capped (`crates/infra/security/src/auth/validation.rs`). There is no ES256/ES384/EdDSA verification path. Audience validation is required by the selected validation policy. First-party session decoding accepts the standard first-party audience set; it does not isolate each API surface by audience.

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
| GET | `/livez` | Public | Liveness: `200` once the port is bound, even while booting. |
| GET | `/readyz` | Public | Readiness: `503` while starting, draining after `SIGTERM`, or when the database probe fails; `200` otherwise. Point load balancers here. |
| GET | `/health` | Public | Health: `200` while starting, then healthy/degraded, `503` when the database is unreachable. |
| GET | `/api/v1/health` | Public | Same as `/health`. |
| GET | `/api/v1/core/oauth/health` | Public | OAuth subsystem liveness. |
| GET | `/api/v1/health/detail` | Authenticated | Detailed health (subsystem status). |
| GET | `/metrics` | Public, separate listener | Prometheus exposition, served only on `server.metrics_port`; not mounted on the public router. Every series carries an `instance` label. |

```bash
curl http://127.0.0.1:8080/readyz
curl http://127.0.0.1:9100/metrics
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
| POST | `/api/v1/core/oauth/register` | Public | Dynamic client registration (RFC 7591). Open only with `security.allow_dynamic_client_registration: true`; otherwise 403 and discovery omits `registration_endpoint`. |
| GET | `/api/v1/core/oauth/register/{client_id}` | Registration token | Read a registered client (RFC 7592; `Authorization: Bearer <registration_access_token>`). |
| PUT | `/api/v1/core/oauth/register/{client_id}` | Registration token | Update a registered client. |
| DELETE | `/api/v1/core/oauth/register/{client_id}` | Registration token | Delete a registered client. |
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

Agents speak the A2A 1.0.1 JSON-RPC binding. Every JSON-RPC request must carry `A2A-Version: 1.0`, as a header or as the `?A2A-Version=1.0` query parameter. A missing or unsupported version returns HTTP 400 with `-32009 VersionNotSupportedError`. Agent-card GETs need no version.

| Method | Params | Result |
|--------|--------|--------|
| `SendMessage` | `message`, optional `configuration` (`returnImmediately`), optional `tenant` | `{"task": …}` or `{"message": …}` |
| `SendStreamingMessage` | as `SendMessage` | SSE stream of `StreamResponse` frames |
| `GetTask` | `id`, optional `historyLength` | `Task` |
| `CancelTask` | `id` | `Task` |
| `SubscribeToTask` | `id` | SSE stream of `StreamResponse` frames |
| `ListTasks` | optional `contextId`, `status`, `pageSize` (1 to 100, default 50), `pageToken`, `historyLength`, `includeArtifacts` | `tasks`, `nextPageToken` (empty on the last page), `pageSize` |

`ListTasks` returns only the caller's own tasks. Push-notification methods return `-32601`. Each stream frame carries the request `id` and exactly one of `task`, `message`, `statusUpdate` or `artifactUpdate`; the stream closing is the terminal signal. Error `data` is an array of `google.rpc.ErrorInfo` objects with `@type`, `reason` (for example `TASK_NOT_FOUND`), `domain: a2a-protocol.org` and `metadata`.

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

Base `/v1` (`crates/entry/api/src/routes/gateway/mod.rs`). The gateway mounts only when analytics and user providers are available. Authentication is resolved per request by the gateway handlers (bearer JWT, API key, bridge PAT or session exchange). The whole group is rate-limited by `rate_limits.gateway_per_second`.

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/v1/` | Gateway root descriptor. |
| GET | `/v1/models` | List available models. |
| POST | `/v1/messages` | Anthropic Messages-shaped inbound request. |
| POST | `/v1/responses` | OpenAI Responses-shaped inbound request. |
| POST | `/v1/otel` | OTLP telemetry ingest (requires a gateway credential and `x-session-id`). |
| POST | `/v1/otel/{rest}` | OTLP telemetry ingest (sub-path; same credential rule). |
| POST | `/v1/auth/bridge/pat` | Exchange a personal access token. |
| POST | `/v1/auth/bridge/session` | Exchange a session credential. |
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

### Scope attribution

Every `/v1/messages` and `/v1/responses` request is attributed to one value in each subject dimension the deployment registers (a `SubjectAttributeProvider`, for example `project` or `cost_centre`; core registers none). For each registered dimension the gateway takes, in order:

1. the `x-systemprompt-scope-<dimension>` header (for example `x-systemprompt-scope-project: apollo`). The value must be one the caller holds in that dimension;
2. the value the API key is bound to in that dimension, checked the same way;
3. the dimension's default for the caller (the provider's first value).

| Condition | Status |
|-----------|--------|
| A header names a dimension no provider registers, or is malformed (`[a-z][a-z0-9_]{0,63}`), empty or repeated | `400` |
| A header or key-bound value is one the caller does not hold (an unknown value is answered the same way) | `403` `not a member of <dimension> '<value>'` |
| A dimension listed in `gateway.require_scopes` resolves to nothing | `400` `scope_required: …` |

The resolved values are written to `ai_request_attributions` (`dimension`, `value`, `source` = `header` \| `api_key` \| `default`) beside the request row, which also records the authenticating `api_key_id`; rejected requests carry the attribution known when they were refused. `x-systemprompt-scope-*` headers are never forwarded to the provider.

### Quota exhaustion

Gateway policies (`services/gateway/policies.yaml`) declare `quota_windows`, each keyed by a `subject`: `user`, `api_key`, or any scope dimension (the window then counts the request's attributed value for that dimension). Admission reserves one request plus the request's estimated input tokens (body length / 4), output tokens (`max_tokens`, clamped to the model's ceiling) and cost (priced with the request's rate card); completion settles the window to the audited usage and a failed or abandoned request returns the reserved tokens and cost. Under `quota_mode: enforce` a breached window answers `429` before the provider is called:

```json
{
  "type": "error",
  "error": {
    "type": "rate_limit_error",
    "message": "cost ceiling exceeded for user window 60s (spent 1500/1000 microdollars)",
    "quota": {
      "window_seconds": 60,
      "subject": "user",
      "dimension": "cost_microdollars",
      "limit": 1000,
      "used": 1500,
      "resets_at": "2026-10-07T10:01:00Z",
      "retry_after_seconds": 37
    }
  }
}
```

`dimension` is one of `requests`, `input_tokens`, `output_tokens`, `cost_microdollars`; it, `limit` and `used` are `null` when the window could not be evaluated under `quota_fault_mode: closed`. The `Retry-After` header equals `retry_after_seconds`, the time until `resets_at`. Under `quota_mode: warn` the request proceeds and the breach is recorded as a `quota` governance decision. Every breach increments `systemprompt_quota_denials_total{subject_kind, dimension, mode}`.

A window is either `window_seconds` long, aligned to the Unix epoch, or a UTC calendar period declared with `calendar: day`, `calendar: week` (starting Monday) or `calendar: month`, in which case `window_seconds` is not read. A calendar window counts one bucket per period and resets at the next period boundary; its `quota.window_seconds` is the length of the current period (for example `2678400` in a 31-day month) and the message names it `calendar-month`:

```yaml
quota_windows:
  - subject: user
    calendar: month
    max_cost_microdollars: 50000000
```

Policies may also declare `rate_windows`, request-rate throttles counted across every replica: `{window_seconds, subject, max_requests}` with `subject` set to `user` or `api_key`. A request past `max_requests` in the current fixed window answers `429` (`rate_limit_error`) with `Retry-After` set to the end of the window, under `quota_mode: warn` as well as `enforce`. An `api_key` window applies only to requests authenticated with a key. If the counter cannot be updated, `quota_fault_mode: closed` answers `503` and `open` admits the request. A request the quota windows then refuse under `enforce` (or whose quota check fails) has its rate-window charge refunded, so only requests that reach the upstream spend request-rate budget; a request refused by the rate window itself still counts.

### API key limits and scope bindings

`POST /api/v1/admin/api-keys` accepts, beside `name`, `target_user_id` and `expires_at`:

| Field | Type | Effect |
|-------|------|--------|
| `model_allowlist` | string[] | Requests for any other model answer `403` before routing. Omit for no restriction; an empty list is refused. |
| `budget_microdollars` | integer | Spend ceiling for the key over `request_window_seconds`. |
| `max_requests` | integer | Request ceiling for the key over `request_window_seconds`. |
| `request_window_seconds` | integer | Required (positive) when either ceiling is set. |
| `scopes` | `[{dimension, value}]` | One bound value per scope dimension; each must be a value the key's owner holds (`403` otherwise, `400` for an unregistered dimension). |

The ceilings run as a quota window with `subject: api_key`, so they reserve, settle and answer `429` exactly as policy windows do. `GET /api/v1/admin/api-keys` returns the same fields.

## Webhooks

See [Context webhooks](#context-webhooks--base-apiv1webhook-authenticated) above. Base `/api/v1/webhook`, authenticated, with `/broadcast`, `/agui`, and `/a2a`.

## Documents

The shared document store ([documents.md](../documents.md)). Mounted only while `files.documents.enabled` is on; otherwise every path answers `404`. All routes are user-authenticated. Bodies are bounded by `files.documents.maxUploadBytes` plus 64 KiB on this router, not the global 2 MiB limit.

| Method | Path | Purpose |
|--------|------|---------|
| GET, POST | `/api/v1/documents` | List by `scope` (`folder`, `shared`, `mine`, `recent`), `folder`, `query`, `sort` (`name`, `updated`, `recent`) and cursor; create from a template (JSON) or import (multipart). |
| GET, HEAD, PATCH, DELETE | `/api/v1/documents/{id}` | Metadata (`ETag` is the quoted head revision id, its content hash in `x-document-content-sha256`); rename or move (`folder_id: null` moves to the root); trash (owner only). |
| POST | `/api/v1/documents/{id}/untrash`, `/copy` | Bring back from the trash (owner); copy into a new document owned by the caller. |
| GET, HEAD, PUT | `/api/v1/documents/{id}/content` | Download with `Range` and conditional headers, at `?revision=` (`ETag` is the quoted revision id, the content hash is in `x-document-content-sha256`); `?format=pdf` serves the revision rendered as `application/pdf` (whole body, the rendering's hash as `ETag`, `.pdf` file name); `?access_token=` from `documents_export` opens it without a session. `PUT` replaces the bytes with `If-Match: "<revision_id>"` (any other form is `400` `invalid_if_match`) and optional `?op_id=`; a stale base merges or answers `409`. |
| POST | `/api/v1/documents/{id}/edits` | Structured edit operations with optional `base_revision`, `op_id`, `answering`, `allow_detach` and `mode` (`land`, `propose`); a proposal answers `201`. |
| GET | `/api/v1/documents/{id}/read` | The `documents_read` views: `view=outline`, `text`, `range`, `cells`, `changes`, `node`, `search`, `meta`; `revision_id`, `verbose`. |
| GET | `/api/v1/documents/{id}/revisions`, `/projection`, `/preview` | History; node projection; sanitised sandboxed HTML preview. |
| GET, POST | `/api/v1/documents/{id}/comments` | Comment threads; `POST .../comments/{comment_id}/resolve` resolves one. |
| GET | `/api/v1/documents/{id}/proposals`, `/proposals/{pid}` | Proposals (`?status=`); one proposal with its changed lines. |
| POST | `/api/v1/documents/{id}/proposals/{pid}/accept`, `/reject` | Review with an optional `note`; `self_review` for one's own. |
| GET, POST, DELETE | `/api/v1/documents/{id}/grants` | Grants; `POST` takes `subject_kind`, `subject_value`, `level`, `valid_until?`, `message?`. |
| GET | `/api/v1/documents/{id}/access` | Owner, grants, inherited grants, the caller's level, open access requests (holders of `share`); `?history=true` adds the grant audit log. |
| POST | `/api/v1/documents/{id}/transfer` | Transfer ownership to an active user (owner only). |
| DELETE | `/api/v1/documents/{id}/grants/all` | Revoke every grant (owner only). |
| POST | `/api/v1/documents/{id}/access-requests` | `{level, message?}`; needs `files.documents.accessRequests`. |
| POST | `/api/v1/documents/{id}/access-requests/{rid}/grant`, `/decline` | Answer a request (holders of `share`). |
| GET | `/api/v1/documents/subject-kinds`, `/subjects` | Accepted grant subject kinds; subject search by `kind`, `q`, `limit` (max 20). |
| PUT, GET | `/api/v1/documents/{id}/presence` | Presence beacon; who is present. |
| POST, DELETE | `/api/v1/documents/{id}/lock` | Lease lock. |
| GET | `/api/v1/documents/{id}/live` | Document-scoped SSE channel (`Last-Event-ID` or `?last_event_id=` resumes). |
| POST | `/api/v1/documents/{id}/collab/awareness` | Publish an awareness state. |
| POST | `/api/v1/documents/{id}/collab/join`, `/updates`, `/leave`, `/cursor` | Live co-editing session routes (`files-collab` feature; `503 collab_engine_unavailable` without an engine). |
| GET, POST | `/api/v1/document-folders` | List; create. |
| PATCH, DELETE | `/api/v1/document-folders/{id}` | Rename, move, set `agent_edit_policy`; delete (`?recursive=true`). |
| GET, POST, DELETE | `/api/v1/document-folders/{id}/grants` | Folder grants. |
| GET | `/api/v1/document-folders/{id}/access` | Folder access read-back (`?history=true`). |
| GET | `/v1/bridge/drive/manifest`, `/changes?since=`, `/blob/{sha256}`, `/proposals` | Bridge Drive feed (device bearer). |
| POST, PUT | `/v1/bridge/drive/commit`, `/presence` | Bridge Drive commit (multipart, `If-Match`) and presence. |

A refused document request answers the common error envelope with `error_key` set to one of the 23 refusal codes shared with the MCP tools and the refusal's typed detail (head and base revisions, node, hash, current text, accepted kinds, owner, existing level, comments, `trashed`, `can_request`) under a `detail` object. Statuses: `409` for `stale_base`, `guard_mismatch`, `anchors_affected`, `proposal_conflict`, `proposal_closed`, `folder_not_empty`, `op_id_mismatch`, `access_request_open`, `access_request_closed` and `not_found` for a node id; `403` for `forbidden` and `self_review`; `404` for `not_found` and `trashed`; `413` for `too_large`; `422` for `integrity_mismatch` and `template_failed`; `423` for `locked`; `400` for `invalid_params`, `unsupported_op`, `unsupported_format`, `unknown_subject` and `access_request_not_needed`. A server-side failure answers `500` with a fixed message and its cause logged. The code table is in [documents.md § Refusals](../documents.md#mcp-tools).

## Other mounted areas

These areas are mounted by `mount_content_and_misc` (`protocol.rs:149`).

| Base path | Auth | Purpose |
|-----------|------|---------|
| `/api/v1/content` | Public + User | Content query and link resolution. |
| `/api/v1/documents`, `/api/v1/document-folders`, `/v1/bridge/drive` | User; device | The document store, listed under [Documents](#documents) above. |
| `/api/v1/sync` | Restricted (Service) | Cloud sync of files. |
| `/api/v1/marketplace` | Public | Marketplace catalog. |
| `/api/v1/analytics` | Admin | Analytics events and streams. |
| `/track/engagement` | Public | Engagement tracking ingest. |
| `/api/v1/admin` | Admin | Admin operations (logs, users, CLI relay, keys). |
| `/auth/link-passkey` | Public | Passkey-linking page. |
