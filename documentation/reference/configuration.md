# Profile Configuration Reference

Complete schema for the `profile.yaml` document that configures a systemprompt-core deployment. Every key, type, default, and constraint below is defined in `crates/shared/models/src/profile/*` and `crates/shared/models/src/services/system_admin.rs`.

A profile lives at `.systemprompt/profiles/<name>/profile.yaml`. It is the single source of truth for configuration; there are no environment-variable fallbacks for profile keys (environment variables are used only for `${VAR}` interpolation and the secrets envelope, documented below).

## Strictness

The top-level `Profile` struct and every nested config struct in this document carry `#[serde(deny_unknown_fields)]` (`crates/shared/models/src/profile/mod.rs:109`). An unrecognized key anywhere in `profile.yaml` is a hard parse error. Do not add keys that are not listed here. The two exceptions that do **not** deny unknown fields are `ProfileInfo` (a runtime status projection, not part of `profile.yaml`) and the secrets document (a JSON file, see [Secrets envelope](#secrets-envelope)).

## Top-level keys

`crates/shared/models/src/profile/mod.rs:108-149`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `name` | string | yes | — | Internal profile identifier. Also drives `profile_style()` (e.g. `local`/`dev`/`development` → development style; `prod`/`production` → production). |
| `display_name` | string | yes | — | Human-readable profile label. |
| `target` | enum `local` \| `cloud` | no | `local` | Deployment target. `cloud` switches path validation to expect `/app`-rooted paths. |
| `site` | object | yes | — | Site metadata. See [`site`](#site). |
| `database` | object | yes | — | Database selector. See [`database`](#database). |
| `server` | object | yes | — | HTTP server binding and headers. See [`server`](#server). |
| `paths` | object | yes | — | Filesystem layout. See [`paths`](#paths). |
| `security` | object | yes | — | JWT/auth settings. See [`security`](#security). |
| `rate_limits` | object | yes | — | Per-route rate limits. See [`rate_limits`](#rate_limits). |
| `system_admin` | object | yes | — | Platform owner identity. See [`system_admin`](#system_admin). |
| `runtime` | object | no | all-defaults | Environment, log level, output. See [`runtime`](#runtime). |
| `cloud` | object | no | absent | Cloud tenant binding. See [`cloud`](#cloud). |
| `secrets` | object | no | absent | Pointer to the secrets document. See [`secrets`](#secrets). |
| `extensions` | object | no | `{ disabled: [] }` | Extension enable/disable. See [`extensions`](#extensions). |
| `gateway` | object | no | absent | Provider-facing inference proxy. See [`gateway`](#gateway). |
| `governance` | object | no | absent | Authorization hook. See [`governance`](#governance). |

`target` accepts the lowercase values `local` and `cloud` (`profile/mod.rs:91`).

## `site`

`crates/shared/models/src/profile/site.rs`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `name` | string | yes | — | Site name. |
| `github_link` | string | no | absent | Optional GitHub repository URL. |

## `database`

`crates/shared/models/src/profile/database.rs`

| Key | YAML key | Type | Required | Default | Meaning |
|-----|----------|------|----------|---------|---------|
| `db_type` | `type` | string | yes | — | Database backend selector. PostgreSQL is the supported backend. |
| `external_db_access` | `external_db_access` | bool | no | `false` | When `true`, the platform uses `external_database_url` from the secrets document if present (`secrets.rs:85`). |

The connection string itself is never in `profile.yaml`; it lives in the secrets document as `database_url` (see [Secrets envelope](#secrets-envelope)).

## `server`

`crates/shared/models/src/profile/server.rs`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `host` | string | yes | — | Bind address (e.g. `127.0.0.1`, `0.0.0.0`). |
| `port` | u16 | yes | — | Bind port. |
| `api_server_url` | string | yes | — | Canonical server URL. |
| `api_internal_url` | string | yes | — | Internal-facing base URL. |
| `api_external_url` | string | yes | — | External-facing base URL. |
| `use_https` | bool | no | `false` | Whether the public scheme is HTTPS. |
| `cors_allowed_origins` | list of string | no | `[]` | Allowed CORS origins. |
| `content_negotiation` | object | no | see below | Markdown content negotiation. |
| `security_headers` | object | no | see below | HTTP security response headers. |
| `instance_id` | string | no | OS hostname / generated short id | Stable replica identifier; empty/unset resolves at config build time. |
| `max_concurrent_streams` | usize | no | `256` | Global cap on concurrent A2A SSE streams for this replica (`config/mod.rs:34`). |
| `trusted_proxies` | list of CIDR string | no | `[]` | Peer CIDRs allowed to set `X-Forwarded-For`, `X-Real-IP`, `CF-Connecting-IP`. Empty means every connection is treated as direct and those headers are ignored. A bare address without `/` is read as `/32` (IPv4) or `/128` (IPv6). |

### `server.content_negotiation`

`crates/shared/models/src/profile/server.rs:54`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `enabled` | bool | no | `false` | Enable Markdown content negotiation. |
| `markdown_suffix` | string | no | `.md` | Suffix that selects the Markdown variant. |

### `server.security_headers`

`crates/shared/models/src/profile/server.rs:77`. Defaults apply when the `security_headers` block is omitted (the struct `Default` sets `enabled: true`).

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `enabled` | bool | no | `true` | Emit security headers. |
| `hsts` | string | no | `max-age=63072000; includeSubDomains; preload` | `Strict-Transport-Security` value. |
| `frame_options` | string | no | `DENY` | `X-Frame-Options` value. |
| `content_type_options` | string | no | `nosniff` | `X-Content-Type-Options` value. |
| `referrer_policy` | string | no | `strict-origin-when-cross-origin` | `Referrer-Policy` value. |
| `permissions_policy` | string | no | `camera=(), microphone=(), geolocation=()` | `Permissions-Policy` value. |
| `content_security_policy` | string | no | absent | `Content-Security-Policy` value; unset means no CSP header. |

## `paths`

`crates/shared/models/src/profile/paths.rs`. Relative paths are resolved against the profile directory at load time (`profile/mod.rs:173`). Required-vs-optional and `/app`-rooting rules differ by `target` (`profile/validation.rs:31-89`).

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `system` | string | yes | — | System root directory. Logs derive as `<system>/logs`. For `target: cloud`, must start with `/app`. |
| `services` | string | yes | — | Services root. Subpaths derive from it: `skills`, `plugins`, `marketplaces`, `hooks`, `agents`, and config files under `config/`, `ai/`, `content/`, `web/`. For `target: cloud`, must start with `/app`. |
| `bin` | string | yes | — | Binary directory. For `target: cloud`, must start with `/app`. |
| `web_path` | string | no | `<system>/web` | Web asset root (parent of `dist/`). For `target: cloud`, must start with `/app/web` and must not contain `/services/web`. |
| `storage` | string | no | absent | File storage root. |
| `geoip_database` | string | no | absent | Path to a MaxMind GeoIP database file. |

## `security`

`crates/shared/models/src/profile/security.rs`. The JWT plane is RS256-only; these keys do not select an algorithm.

| Key | YAML key | Type | Required | Default | Meaning |
|-----|----------|------|----------|---------|---------|
| `issuer` | `jwt_issuer` | string | yes | — | JWT `iss` claim issued and expected by this deployment. |
| `access_token_expiration` | `jwt_access_token_expiration` | i64 (seconds) | yes | — | Access token lifetime. |
| `refresh_token_expiration` | `jwt_refresh_token_expiration` | i64 (seconds) | yes | — | Refresh token lifetime. |
| `audiences` | `jwt_audiences` | list of audience | yes | — | Accepted JWT audiences. Each entry is one of `web`, `api`, `a2a`, `mcp`, `internal`, `bridge`, `hook`, or an arbitrary resource string (`auth/enums.rs:10`). |
| `allowed_resource_audiences` | `allowed_resource_audiences` | list of string | no | `[]` | Additional resource audience identifiers permitted. |
| `allow_registration` | `allow_registration` | bool | no | `true` | Whether self-service OAuth client/user registration is open. |
| `signing_key_path` | `signing_key_path` | path | no | `signing_key.pem` | Path to the RS256 signing key (PEM). |
| `trusted_issuers` | `trusted_issuers` | list of object | no | `[]` | Federated issuers accepted in addition to `jwt_issuer`. |

`validate_aud` is currently `false` in the validation plane; audience isolation is not enforced. Do not configure on the assumption it is.

### `security.trusted_issuers[]`

`crates/shared/models/src/profile/security.rs:42`

| Key | Type | Required | Meaning |
|-----|------|----------|---------|
| `issuer` | string | yes | Trusted issuer `iss` value. |
| `jwks_uri` | string | yes | JWKS endpoint for that issuer. |
| `audience` | string | yes | Audience expected from that issuer. |

## `rate_limits`

`crates/shared/models/src/profile/rate_limits.rs`. Values are requests per second per route group; `burst_multiplier` scales the burst allowance. All keys default, so the block may be `{}`.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `disabled` | bool | no | `false` | Disable all rate limiting. |
| `oauth_public_per_second` | u64 | no | `10` | Public OAuth endpoints. |
| `oauth_auth_per_second` | u64 | no | `10` | Authenticated OAuth endpoints. |
| `contexts_per_second` | u64 | no | `100` | `/api/v1/core/contexts`. |
| `tasks_per_second` | u64 | no | `50` | `/api/v1/core/tasks`. |
| `artifacts_per_second` | u64 | no | `50` | `/api/v1/core/artifacts`. |
| `agent_registry_per_second` | u64 | no | `50` | Agent registry. |
| `agents_per_second` | u64 | no | `20` | Per-agent routes. |
| `mcp_registry_per_second` | u64 | no | `50` | MCP registry. |
| `mcp_per_second` | u64 | no | `200` | MCP server routes. |
| `stream_per_second` | u64 | no | `100` | SSE stream routes. |
| `content_per_second` | u64 | no | `50` | Content routes. |
| `burst_multiplier` | u64 | no | `3` | Burst allowance multiplier. |
| `tier_multipliers` | object | no | see below | Per-caller-tier scaling. |

### `rate_limits.tier_multipliers`

`crates/shared/models/src/profile/rate_limits.rs:5`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `admin` | f64 | no | `10.0` | Admin tier multiplier. |
| `user` | f64 | no | `1.0` | Authenticated user multiplier. |
| `a2a` | f64 | no | `5.0` | A2A caller multiplier. |
| `mcp` | f64 | no | `5.0` | MCP caller multiplier. |
| `service` | f64 | no | `5.0` | Service caller multiplier. |
| `anon` | f64 | no | `0.5` | Anonymous caller multiplier. |

## `system_admin`

`crates/shared/models/src/services/system_admin.rs:19`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `username` | string | yes | — | Username of the platform owner. Resolved at bootstrap against the `users` table; the row must exist, be active, and hold the `admin` role, or the platform refuses to boot. |

## `runtime`

`crates/shared/models/src/profile/runtime.rs`. Whole block defaults; may be omitted.

| Key | Type | Required | Default | Allowed values | Meaning |
|-----|------|----------|---------|----------------|---------|
| `environment` | enum | no | `development` | `development`, `test`, `staging`, `production` | Deployment environment. |
| `log_level` | enum | no | `normal` | `quiet`, `normal`, `verbose`, `debug` | Maps to tracing filter `error`/`info`/`debug`/`trace` (`runtime.rs:104`). |
| `output_format` | enum | no | `text` | `text`, `json`, `yaml` | CLI output format. |
| `no_color` | bool | no | `false` | Disable ANSI color. |
| `non_interactive` | bool | no | `false` | Disable interactive prompts. |

## `cloud`

`crates/shared/models/src/profile/cloud.rs`. Optional; absent means a local (non-cloud) deployment.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `tenant_id` | TenantId (string) | no | absent | Cloud tenant identifier. A value prefixed `local_` marks a local trial. |
| `validation` | enum `strict` \| `warn` \| `skip` | no | `strict` | Cloud credential validation mode. `warn` or `skip` also marks a local trial. |

## `secrets`

`crates/shared/models/src/profile/secrets.rs`. Points the bootstrap at the secrets document; it does not contain secret values.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `secrets_path` | string | yes | — | Path to the secrets file, resolved relative to the profile directory (with `~/` home expansion). |
| `source` | enum `file` \| `env` | yes | — | Where to read secrets from. `file` reads `secrets_path`; `env` reads environment variables (with a file-first fallback when not in a Fly.io container — `secrets/loader.rs:44`). |
| `validation` | enum `strict` \| `warn` \| `skip` | no | `warn` | How a failed file load is handled. |

## `extensions`

`crates/shared/models/src/profile/mod.rs:58`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `disabled` | list of string | no | `[]` | Extension IDs to disable. An ID in this list is excluded from registration. |

See [`concepts/extensions.md`](../concepts/extensions.md) for the extension model.

## `gateway`

`crates/shared/models/src/profile/gateway.rs`. The provider-facing inference proxy. Optional; absent means the gateway is off.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `enabled` | bool | no | `false` | Enable the gateway. |
| `routes` | list of object | no | `[]` | Inline model→provider routes. See [`gateway.routes[]`](#gatewayroutes). |
| `catalog` | object | no | absent | Providers and models. Either inline (`{ providers: [...], models: [...] }`) or file-backed (`{ path: "./catalog.yaml" }`). See [Gateway catalog](#gateway-catalog). |
| `auth_scheme` | string | no | `bearer` | Upstream auth scheme. |
| `inference_path_prefix` | string | no | `/v1` | Path prefix the gateway serves provider-facing inference under. |

Every route and catalog provider endpoint is validated through the shared outbound-URL guard (`net::validate_outbound_url`), which rejects loopback and private-network destinations to prevent the proxy becoming an SSRF primitive (`gateway.rs:57`).

### `gateway.routes[]`

`crates/shared/models/src/profile/gateway.rs:184`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `id` | string | no | derived | Stable route id; synthesized from pattern+provider+endpoint when empty. |
| `model_pattern` | string | yes | — | Match pattern. `*` matches all; `prefix*` and `*suffix` are supported; otherwise exact. |
| `provider` | string | yes | — | Provider name. |
| `endpoint` | string | yes | — | Upstream endpoint URL (SSRF-guarded). |
| `api_key_secret` | string | yes | — | Name of the secret holding the upstream API key. |
| `upstream_model` | string | no | requested model | Override model name sent upstream. |
| `extra_headers` | map<string,string> | no | `{}` | Additional upstream request headers. |
| `pricing` | object | no | absent | Optional model pricing metadata. |

### Gateway catalog

`crates/shared/models/src/profile/gateway/catalog.rs`. The catalog content — whether inlined under `gateway.catalog:` or loaded from the file referenced by `gateway.catalog.path` — has the same shape: a document with `providers` and `models`.

`providers[]`:

| Key | Type | Required | Meaning |
|-----|------|----------|---------|
| `name` | string | yes | Provider name (referenced by models). |
| `endpoint` | string | yes | Upstream endpoint (SSRF-guarded). |
| `api_key_secret` | string | yes | Secret name for the upstream API key. |
| `extra_headers` | map<string,string> | no | Additional upstream headers. |

`models[]`:

| Key | Type | Required | Meaning |
|-----|------|----------|---------|
| `id` | string | yes | Model id exposed by the gateway (must be non-empty). |
| `provider` | string | yes | Provider name; must match a declared provider. |
| `display_name` | string | no | Human-readable label. |
| `upstream_model` | string | no | Override model name sent upstream. |
| `pricing` | object | no | Pricing metadata. |

## `governance`

`crates/shared/models/src/profile/governance.rs`. Configures the authorization hook for the gateway and MCP planes. The hook is fail-closed: an absent `governance` block, absent `authz`, or unparseable config installs a deny-all hook.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `authz` | object | no | absent | Authorization hook config. See [`governance.authz.hook`](#governanceauthzhook). |

### `governance.authz.hook`

`crates/shared/models/src/profile/governance.rs:53`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `mode` | enum `webhook` \| `disabled` \| `unrestricted` | yes | — | `webhook`: POST each request to `url`; any transport error, non-2xx, or decode failure denies. `disabled`: deny every request (surface installed). `unrestricted`: allow every request — test/dev only. |
| `url` | string | conditional | absent | Required for `webhook` mode. |
| `timeout_ms` | u64 | no | `500` | Per-request hook timeout. |
| `acknowledgement` | string | conditional | absent | Required for `unrestricted` mode; must equal the literal `I understand this disables all authorization` or bootstrap errors. |

```yaml
governance:
  authz:
    hook:
      mode: webhook
      url: http://localhost:8080/api/public/govern/authz
      timeout_ms: 500
```

## `${VAR}` interpolation

Before YAML parsing, the loader substitutes `${VAR}` tokens in the raw profile text with the value of the environment variable `VAR` (`profile/mod.rs:79-86`). The pattern is exactly `${WORD}` (alphanumeric and underscore). An undefined variable is left literally in place (the `${VAR}` text is preserved, not blanked). Interpolation is textual and applies to any value in the document.

```yaml
server:
  api_external_url: ${PUBLIC_BASE_URL}
```

## Secrets envelope

Secret values are never stored in `profile.yaml`. They live in a separate JSON document referenced by the [`secrets`](#secrets) section, loaded by `SecretsBootstrap`. The envelope is customer-owned; the binary never holds a master key. Schema in `crates/shared/models/src/secrets.rs`.

| Field | JSON key | Type | Required | Notes |
|-------|----------|------|----------|-------|
| `oauth_at_rest_pepper` | `oauth_at_rest_pepper` | string | yes | Minimum 32 characters (`secrets.rs:13`); shorter values fail validation. |
| `database_url` | `database_url` | string | yes | Primary connection string. |
| `manifest_signing_secret_seed` | `manifest_signing_secret_seed` | string (base64, 32 bytes) | no | Ed25519 manifest signing seed. Generated and persisted at bootstrap if missing and the file is writable; ephemeral for the boot otherwise. |
| `database_write_url` | `database_write_url` | string | no | Separate write endpoint. |
| `external_database_url` | `external_database_url` | string | no | Used when `database.external_db_access` is `true`. |
| `internal_database_url` | `internal_database_url` | string | no | Internal connection string. |
| `gemini` | `gemini` | string | no | Gemini API key. |
| `anthropic` | `anthropic` | string | no | Anthropic API key. |
| `openai` | `openai` | string | no | OpenAI API key. |
| `github` | `github` | string | no | GitHub token. |
| `moonshot` | `moonshot` | string | no | Moonshot/Kimi key. |
| `qwen` | `qwen` | string | no | Qwen/DashScope key. |
| (custom) | any other key | string | no | Extra keys flatten into a `custom` map and are addressable by name. |

`null` values are stripped before deserialization (`secrets.rs:62`).

### Environment-variable source

When `secrets.source` is `env` (or a Fly.io container is detected via `FLY_APP_NAME`), secrets are read from environment variables instead of the file (`secrets/loader.rs`). Mapping:

| Secret | Environment variable |
|--------|----------------------|
| `oauth_at_rest_pepper` | `OAUTH_AT_REST_PEPPER` (required) |
| `database_url` | `DATABASE_URL` (required) |
| `manifest_signing_secret_seed` | `MANIFEST_SIGNING_SECRET_SEED` |
| `database_write_url` | `DATABASE_WRITE_URL` |
| `external_database_url` | `EXTERNAL_DATABASE_URL` |
| `internal_database_url` | `INTERNAL_DATABASE_URL` |
| `gemini` | `GEMINI_API_KEY` |
| `anthropic` | `ANTHROPIC_API_KEY` |
| `openai` | `OPENAI_API_KEY` |
| `github` | `GITHUB_TOKEN` |
| `moonshot` | `MOONSHOT_API_KEY` or `KIMI_API_KEY` |
| `qwen` | `QWEN_API_KEY` or `DASHSCOPE_API_KEY` |
| (custom) | names listed in `SYSTEMPROMPT_CUSTOM_SECRETS` (comma-separated) |

## Bootstrap order

Configuration is assembled in a fixed sequence; later stages depend on earlier ones (`crates/infra/config/src/bootstrap/`). The type-state `BootstrapSequence` enforces that secrets cannot initialize before the profile (`bootstrap/mod.rs:48-93`).

1. **ProfileBootstrap** — reads `profile.yaml`, performs `${VAR}` interpolation, resolves relative paths, deserializes into `Profile`.
2. **SecretsBootstrap** — loads the secrets document referenced by the profile (file or env), validates required fields (e.g. the 32-char pepper), ensures the manifest signing seed.
3. **CredentialsBootstrap** — loads cloud credentials (file, or environment in a Fly.io container) and validates them against the cloud API for `cloud` targets (`crates/infra/cloud/src/credentials_bootstrap/`).
4. **Config** — composes the validated profile and resolved secrets into the runtime config.
5. **AppContext** — constructs the application context (database pool, services, resolved system admin) consumed by the API and CLI.

## Minimal local profile

A complete, parsing local profile with required keys and representative defaults:

```yaml
name: local
display_name: Local Development
target: local
site:
  name: My systemprompt instance
database:
  type: postgres
server:
  host: 127.0.0.1
  port: 8080
  api_server_url: http://127.0.0.1:8080
  api_internal_url: http://127.0.0.1:8080
  api_external_url: http://127.0.0.1:8080
paths:
  system: /var/www/html/myapp
  services: /var/www/html/myapp/services
  bin: /var/www/html/myapp/bin
security:
  jwt_issuer: http://127.0.0.1:8080
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 2592000
  jwt_audiences: [web, api, a2a, mcp]
rate_limits: {}
system_admin:
  username: admin
secrets:
  secrets_path: ../secrets/local.secrets.json
  source: file
```

The matching secrets file:

```json
{
  "oauth_at_rest_pepper": "a-string-of-at-least-thirty-two-chars",
  "database_url": "postgresql://user:pass@localhost:5432/db"
}
```

## Related

- [HTTP API reference](./http-api.md) — routes whose rate limits are configured above.
- [Extensions](../concepts/extensions.md) — what the `extensions.disabled` list controls.
- [Feature flags](./feature-flags.md) — facade build features that bring in the API, CLI, and gateway code paths.
