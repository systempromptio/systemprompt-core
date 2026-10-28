# Profile Configuration Reference

Complete schema for the `profile.yaml` document that configures a systemprompt-core deployment. Every key, type, default, and constraint below is defined in `crates/shared/manifest/src/profile/*` and `crates/shared/manifest/src/services/system_admin.rs`.

A profile is the overlay `.systemprompt/profiles/<name>/profile.yaml` layered over the optional shared `.systemprompt/profiles/base.yaml` (`crates/shared/manifest/src/profile/layered.rs`). The base holds what every environment shares; the overlay holds only what differs (URLs, tenant, sources, log level, `allow_registration`). Maps merge key by key with the overlay winning; a sequence or scalar in the overlay replaces the base value whole, so lists are never concatenated. A missing base is fine; a missing overlay is an error. `admin config` setters and `cloud profile edit` write only the changed keys into the overlay, never the base. A pre-0.73 shape (a top-level `providers:`, `gateway:` or `evaluation:` key, or a `*.secrets.profile.yaml` file) is refused with a hint to run `systemprompt admin config migrate`. The profile is the single source of truth for configuration; there are no environment-variable fallbacks for profile keys (environment variables are used only for `${VAR}` interpolation and the secrets envelope, documented below).

## Strictness

The top-level `Profile` struct and every nested config struct in this document carry `#[serde(deny_unknown_fields)]` (`crates/shared/manifest/src/profile/mod.rs:109`). An unrecognized key anywhere in `profile.yaml` is a hard parse error. Do not add keys that are not listed here. The one exception that does **not** deny unknown fields is the secrets document (a JSON file, see [Secrets envelope](#secrets-envelope)).

## Top-level keys

`crates/shared/manifest/src/profile/mod.rs:108-149`

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
| `secrets` | object | no | absent | Where secrets come from: a file, the environment, or Vault. See [`secrets`](#secrets). |
| `services` | object | no | all-defaults | Where the services tree comes from. See [`services`](#services). |
| `extensions` | object | no | `{ disabled: [] }` | Extension enable/disable. See [`extensions`](#extensions). |
| `governance` | object | no | absent | Authorization hook. See [`governance`](#governance). |
| `observability` | object | no | `{ otlp: null }` | Where this instance exports its own telemetry. See [`observability`](#observability). |

`target` accepts the lowercase values `local` and `cloud` (`profile/mod.rs:91`).

Two keys belong to the services tree and are rejected in a profile with a message naming
their home: `providers` and `gateway`. See [Provider catalog and gateway — moved to the services
tree](#provider-catalog-and-gateway--moved-to-the-services-tree).

## `site`

`crates/shared/manifest/src/profile/site.rs`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `name` | string | yes | — | Site name. |
| `github_link` | string | no | absent | Optional GitHub repository URL. |

## `database`

`crates/shared/manifest/src/profile/database.rs`

| Key | YAML key | Type | Required | Default | Meaning |
|-----|----------|------|----------|---------|---------|
| `db_type` | `type` | string | yes | — | Database backend selector. PostgreSQL is the supported backend. |
| `external_db_access` | `external_db_access` | bool | no | `false` | When `true`, the platform uses `external_database_url` from the secrets document if present (`secrets.rs:85`). |

| `pool` | `pool` | object | no | engine defaults | Connection-pool sizing: `max_connections` (1–500, default 50), `max_connections_by_role` (`all`, `gateway`, `api`, `worker`, `runner`; each 1–500, overrides `max_connections` for that `server.role`), `session_max_connections` (1–500, default 10; sizes the pool `database_session_url` opens), `acquire_timeout_secs`, `idle_timeout_secs`, `max_lifetime_secs`, `statement_cache_capacity`. See [guides/database-connections.md](../guides/database-connections.md). |

The connection string itself is never in `profile.yaml`; it lives in the secrets document as `database_url` (see [Secrets envelope](#secrets-envelope)).

## `server`

`crates/shared/manifest/src/profile/server.rs`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `host` | string | yes | — | Bind address (e.g. `127.0.0.1`, `0.0.0.0`). |
| `port` | u16 | yes | — | Bind port. |
| `external_url` | string | yes | — | The one public URL (`http://` or `https://`). The API server/external URLs, `use_https`, the JWT issuer and the first CORS origin are derived from it at load. |
| `extra_cors_origins` | list of string | no | `[]` | CORS origins allowed in addition to the origin of `external_url`. |

Derived, never stated (`crates/shared/manifest/src/profile/derived.rs`): `api_server_url`, `api_external_url`, `use_https`, `cors_allowed_origins` and `security.jwt_issuer` come from `external_url`; `api_internal_url` (`http://<host>:<port>`, with `0.0.0.0`/`::` read as `localhost`) and `governance.authz.hook.url` (`<api_internal_url>/api/public/govern/authz` in `webhook` mode) come from `host` and `port`. A profile that states any of them is refused at load with an error naming the field and its source.
| `content_negotiation` | object | no | see below | Markdown content negotiation. |
| `security_headers` | object | no | see below | HTTP security response headers. |
| `instance_id` | string | no (required on `cloud` unless `HOSTNAME` is set) | `HOSTNAME`, else a random id on local targets | Stable replica identifier. Keys the service registry, event outbox origin and node-scoped scheduler runs; stamped on logs, `ai_requests`, metrics and `x-served-by`. A cloud profile that resolves to neither refuses to boot. |
| `metrics_port` | u16 | no | unset | Serve `GET /metrics` on a second listener bound to `host:metrics_port`. Unset means no metrics endpoint. Must differ from `port`. |
| `max_concurrent_streams` | usize | no | `256` | Global cap on concurrent A2A SSE streams for this replica (`config/mod.rs:34`). |
| `role` | `all` \| `gateway` \| `api` \| `worker` \| `runner` | no | `all` | Which surfaces and background work the node runs; see [deploy-production.md §3.1](../guides/deploy-production.md). `admin` is not a role: a profile that names it is refused with an error naming `api` and `worker`. `api` and `worker` with `storage.backend: local` require `storage.shared: true`. |
| `advertise_url` | string | required on `worker` | unset | Absolute `http(s)` URL peers use to reach this node (e.g. `http://${POD_IP}:8080`). A worker records it with every registry heartbeat; `api` replicas forward MCP and agent traffic to the worker that owns the process through it. |
| `trusted_proxies` | list of CIDR string | required on `cloud` | `[]` | Peer CIDRs allowed to set `X-Forwarded-For`, `X-Real-IP`, `CF-Connecting-IP`. Empty means every connection is treated as direct and those headers are ignored. A bare address without `/` is read as `/32` (IPv4) or `/128` (IPv6). |
| `shutdown` | object | no | see below | Shutdown deadlines. |

### `server.shutdown`

`crates/shared/manifest/src/profile/shutdown.rs`. Unknown keys are refused. The server logs `pre_stop_delay_secs + drain_grace_secs + 2 + forced_exit_grace_secs` at boot (the 2 s is the fixed flush window for streams ended by the drain); an orchestrator's termination grace must exceed it.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `pre_stop_delay_secs` | u64 | no | `0` | After the termination signal `/readyz` answers `503 draining` while the listener keeps accepting for this long, so endpoints can update before connections are refused. At most `600`. |
| `drain_grace_secs` | u64 | no | `10` | How long in-flight requests and SSE streams may run once the listener stops accepting. `1`–`86400`. A gateway stream still open when it expires ends itself with a terminal error frame in its wire format ("server is shutting down; retry the request"), counted in `gateway_streams_aborted_on_shutdown_total`, and gets 2 s to flush. |
| `child_grace_secs` | u64 | no | `5` | How long MCP and agent children get to exit after their signal. Greater than `0`. |
| `background_grace_secs` | u64 | no | `5` | How long owned background work, including the final gateway journal settlement, gets to finish. Greater than `0`. |
| `forced_exit_grace_secs` | u64 | no | `10` | Backstop armed after the drain; the process exits non-zero when it fires. Must exceed `child_grace_secs` and `background_grace_secs`. |

### `server.content_negotiation`

`crates/shared/manifest/src/profile/server.rs:54`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `enabled` | bool | no | `false` | Enable Markdown content negotiation. |
| `markdown_suffix` | string | no | `.md` | Suffix that selects the Markdown variant. |

### `server.security_headers`

`crates/shared/manifest/src/profile/server.rs:77`. Defaults apply when the `security_headers` block is omitted (the struct `Default` sets `enabled: true`).

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

`crates/shared/manifest/src/profile/paths.rs`. Relative paths are resolved against `root` at load time; `root` itself is relative to the overlay's directory and defaults to it. `admin config validate --profile` refuses a committed profile layer that names an absolute temp path (`/tmp/…`, `/var/tmp/…`). Required-vs-optional and `/app`-rooting rules differ by `target` (`profile/validation.rs:31-89`).

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `root` | string | no | the overlay's directory | The one root every other relative path resolves against. |
| `system` | string | yes | — | System root directory. Logs derive as `<system>/logs`. For `target: cloud`, must start with `/app`. |
| `services` | string | yes | — | Services root. Subpaths derive from it: `skills`, `plugins`, `marketplaces`, `hooks`, `agents`, and config files under `config/`, `ai/`, `content/`, `web/`. For `target: cloud`, must start with `/app`. |
| `bin` | string | yes | — | Binary directory. For `target: cloud`, must start with `/app`. |
| `web_path` | string | no | `<system>/web` | Web asset root (parent of `dist/`). For `target: cloud`, must start with `/app/web` and must not contain `/services/web`. |
| `storage` | string | no | absent | File storage root. |
| `geoip_database` | string | no | absent | Path to a MaxMind GeoIP database file. |
| `shipped_files` | string | no | absent | Read-only directory consulted for `/files/*` after `<storage>/files` and before the storage backend. |
| `data` | string | no | `<storage>/data` | Node-local root for state the server writes for itself: the gateway accounting journal and `scratch/` working trees. Required when `storage.shared: true`, so each replica keeps its own journal off the shared mount. Must not be empty. |

## `storage`

`crates/shared/manifest/src/profile/storage.rs`. Optional; the section may be omitted. Unknown keys and unknown backends are rejected.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `backend` | `local` \| `gcs` \| `s3` | no | `local` | Storage backend for uploads and generated images. `local` writes under `paths.storage`, which must then be set; `gcs` writes to a Cloud Storage bucket; `s3` writes to an S3-compatible bucket. Files in a bucket are served at their `/files/...` URL by every replica. |
| `shared` | bool | no | `false` | Declare that `paths.storage` is one mount every replica sees. Boot probes the root by writing a per-instance marker and reading it back (failure refuses to boot), and warns when `shared` disagrees with the markers other replicas left. With `false` those files are node-local, which only a single replica can serve. |
| `bucket` | string | `gcs`, `s3` | — | Bucket name. |
| `prefix` | string | no | absent | Object-name prefix inside the bucket. |
| `public_read` | bool | no | `false` | Return the bucket URL as the stored file's public URL. |
| `credentials` | `workload_identity` \| `{ secret: NAME }` | `s3` | `workload_identity` (`gcs`) | `gcs`: metadata-server identity or a service-account key secret. `s3`: a secret holding `access_key_id`, `secret_access_key` and optional `session_token`. |
| `endpoint` | URL | no | `https://s3.<region>.amazonaws.com` | `s3` only. |
| `region` | string | no | `us-east-1` | `s3` only; the SigV4 signing region. |
| `path_style` | bool | no | `false` | `s3` only; address the bucket as `<endpoint>/<bucket>/<key>`. Required when `bucket` contains `_` or `.`. |

The S3 credentials need `s3:GetObject`, `s3:PutObject` and `s3:DeleteObject` on the prefix, and `s3:ListBucket` on the bucket. Without `s3:ListBucket`, S3 answers a missing key with `403` instead of `404`, so a request for a missing `/files/...` URL returns `500` instead of `404`.

File records hold the path relative to `paths.storage` (for example `files/uploads/contexts/<ctx>/images/<id>.png`); public URLs and the on-disk layout are unchanged.

## `security`

`crates/shared/manifest/src/profile/security.rs`. The JWT plane is RS256-only; these keys do not select an algorithm.

| Key | YAML key | Type | Required | Default | Meaning |
|-----|----------|------|----------|---------|---------|
| `issuer` | — | string | derived | — | JWT `iss` claim issued and expected by this deployment. Derived from `server.external_url`; stating `jwt_issuer` is a load error. |
| `access_token_expiration` | `jwt_access_token_expiration` | i64 (seconds) | yes | — | Access token lifetime. |
| `refresh_token_expiration` | `jwt_refresh_token_expiration` | i64 (seconds) | yes | — | Refresh token lifetime. |
| `audiences` | `jwt_audiences` | list of audience | yes | — | Accepted JWT audiences. Each entry is one of `web`, `api`, `a2a`, `mcp`, `internal`, `bridge`, `hook`, or an arbitrary resource string (`auth/enums.rs:10`). |
| `allowed_resource_audiences` | `allowed_resource_audiences` | list of string | no | `[]` | Additional resource audience identifiers permitted. |
| `allow_registration` | `allow_registration` | bool | no | `false` | Whether self-service passkey (user) registration is open. |
| `allow_dynamic_client_registration` | `allow_dynamic_client_registration` | bool | no | `false` | Whether RFC 7591 dynamic client registration is open. Remote MCP clients that register themselves depend on it; set `true` explicitly when they connect, leave it `false` when clients are pre-provisioned. |
| `api_keys.max_lifetime_days` | `api_keys.max_lifetime_days` | integer | no | unset | Maximum lifetime of a personal access token, in days (must be greater than 0). Applies to every issuance path (admin API, CLI, bridge sign-in, agent runs): a key issued without an expiry gets the maximum, and a longer expiry is rejected. Unset means no ceiling. |
| `signing_key_path` | `signing_key_path` | path | no | `signing_key.pem` | Path to the RS256 signing key (PEM). |
| `trusted_issuers` | `trusted_issuers` | list of object | no | `[]` | Federated issuers accepted in addition to `jwt_issuer`. |

Audience validation requires a nonempty policy. The first-party session policy accepts the standard first-party audience set; use resource-specific policies where narrower audience isolation is required.

### `security.trusted_issuers[]`

`crates/shared/manifest/src/profile/security.rs:42`

| Key | Type | Required | Meaning |
|-----|------|----------|---------|
| `issuer` | string | yes | Trusted issuer `iss` value. |
| `jwks_uri` | string | yes | JWKS endpoint for that issuer. |
| `audience` | string | yes | Audience expected from that issuer. |

## `rate_limits`

`crates/shared/manifest/src/profile/rate_limits.rs`. Values are requests per second per route group; `burst_multiplier` scales the burst allowance. All keys default, so the block may be `{}`.

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
| `gateway_per_second` | u64 | no | `100` | Inference gateway routes (`/api/v1/gateway/*`, including OTLP ingest and bridge credential exchange). |
| `burst_multiplier` | u64 | no | `3` | Burst allowance multiplier. |

Limits are enforced per caller: a request carrying a signature-verified identity is bucketed by
that identity, and an unauthenticated request by the client address resolved through
`trusted_proxies`. Hop headers from an untrusted peer are ignored, so a caller cannot select its
own bucket. Identity-keyed budgets are counted in the database and therefore hold across every
replica; address-keyed throttles are per process, so their effective ceiling scales with the
replica count. The identity-keyed window fails closed: if the database counter cannot be
updated, an authenticated request answers `503` with `Retry-After` instead of running unmetered.
There is no fail-open option; `disabled: true` is the only way to run without the window.

## `system_admin`

`crates/shared/manifest/src/services/system_admin.rs:19`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `username` | string | yes | — | Username of the platform owner. Resolved at bootstrap against the `users` table; the row must exist, be active, and hold the `admin` role, or the platform refuses to boot. |

## `runtime`

`crates/shared/manifest/src/profile/runtime.rs`. Whole block defaults; may be omitted.

| Key | Type | Required | Default | Allowed values | Meaning |
|-----|------|----------|---------|----------------|---------|
| `environment` | enum | no | `development` | `development`, `test`, `staging`, `production` | Deployment environment. |
| `log_level` | enum | no | `normal` | `quiet`, `normal`, `verbose`, `debug` | Maps to tracing filter `error`/`info`/`debug`/`trace` (`runtime.rs:104`). |
| `output_format` | enum | no | `text` | `text`, `json`, `yaml` | CLI output format. |
| `no_color` | bool | no | `false` | Disable ANSI color. |
| `non_interactive` | bool | no | `false` | Disable interactive prompts. |

## `cloud`

`crates/shared/manifest/src/profile/cloud.rs`. Optional; absent means a local (non-cloud) deployment.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `tenant_id` | TenantId (string) | no | absent | Cloud tenant identifier. A value prefixed `local_` marks a local trial. |
| `validation` | enum `strict` \| `warn` \| `skip` | no | `strict` | Cloud credential validation mode. `warn` or `skip` also marks a local trial. |

## `secrets`

`crates/shared/manifest/src/profile/secrets.rs`. Points the bootstrap at the secrets document; it does not contain secret values.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `source` | enum `file` \| `env` \| `vault` | yes | — | Where to read secrets from. `file` reads `secrets_path`; `env` reads environment variables (with a file-first fallback when not on a deployment host); `vault` reads a KV v2 document. |
| `secrets_path` | string | for `file` | — | Path to the secrets file, resolved relative to the profile directory (with `~/` home expansion). Required for `source: file`; unused for `source: vault`. |
| `validation` | enum `strict` \| `warn` \| `skip` | no | `warn` | How a loaded document is checked. It does not govern whether a source may be abandoned: a Vault fetch failure aborts the boot under every mode. |
| `vault` | object | for `vault` | absent | KV v2 access. Required when `source: vault`, and refused for the other two sources. See [`secrets.vault`](#secretsvault). |
| `refresh_secs` | u64 | no | absent | `source: file` only. Re-read the secrets file on this interval (minimum `60`), so a rotated Kubernetes Secret mounted as a volume reaches the running process without a restart. Every replica and every role reloads its own store; there is no cluster lock. Pinned keys (`encryption_master_key`, `oauth_at_rest_pepper`, `manifest_signing_secret_seed`, `database_url`, `signing_key_pem`, `jwt_secret`) still require a restart: a reload that changes one is refused. An `external` source sets its interval on `secrets.external.refresh_secs` instead. |

### `secrets.vault`

`crates/shared/manifest/src/profile/vault.rs`. See [`guides/vault-secrets.md`](../guides/vault-secrets.md) for the task-oriented version.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `address` | string | yes | — | Vault or OpenBao base URL. Validated by the outbound-URL guard, so a private literal needs the host in `SYSTEMPROMPT_TRUSTED_HTTP_HOSTS`. Redirects are not followed. |
| `mount` | string | no | `secret` | KV v2 mount name. |
| `path` | string | yes | — | Path of the document holding the `secrets.json` shape. |
| `namespace` | string | no | absent | Sent as `X-Vault-Namespace`. |
| `auth` | object | yes | — | Login method; tagged by `method`. See below. |
| `keys` | map of string → `{path, field}` | no | `{}` | Per-key overrides read after the base document. Each names a KV path and the field to take from it. A named field that does not exist is an error. |
| `ca_cert_path` | string | no | absent | PEM bundle for a private CA. There is no TLS-verification bypass; `deny_unknown_fields` makes `skip_verify` a parse error. |
| `timeout_secs` | integer | no | `10` | Per-request timeout; maximum `120`. |
| `retries` | integer | no | `3` | Bounded retries on connect failures, 5xx and 429 only; maximum `10`. |

`auth.method` is one of:

| `method` | Keys | Defaults |
|-----|------|----------|
| `token` | `token_env`, `token_file` | `VAULT_TOKEN`; `token_file` absent |
| `approle` | `role_id_env`, `secret_id_env`, `mount` | `VAULT_ROLE_ID`, `VAULT_SECRET_ID`, `approle` |
| `kubernetes` | `role` (required), `jwt_path`, `mount` | `/var/run/secrets/kubernetes.io/serviceaccount/token`, `kubernetes` |

The token is never a profile key. Writing `${VAULT_TOKEN}` into `profile.yaml` is forbidden; the token comes from the environment, a file, or a login. Child processes are handed the resolved secrets and never inherit Vault credentials.

**Source precedence** (`crates/infra/config/src/bootstrap/secrets/resolve.rs`), highest first: subprocess environment; `source: vault`, including on deployment hosts; deployment host environment; `source: env` locally, file first; `source: file`.

## `services`

`crates/shared/manifest/src/profile/services.rs`. Where the services tree comes from. An absent block, or one with no `sources`, serves the tree at `paths.services` — the default. See [`guides/services-bundles.md`](../guides/services-bundles.md).

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `sources` | list of object | no | `[]` | Bundles composed, in order, into the served tree. Empty means the baked tree. |
| `cache_dir` | string | no | `<paths.system>/services-cache` | Where fetched and composed bundles live. Must be absolute, and under `/app` for `target: cloud`. |
| `on_fetch_failure` | enum `fail_closed` \| `use_last_good` \| `use_bundled` | no | `use_last_good` | What to serve when a source cannot be fetched or fails verification. Every path logs the failure and is reported by the status endpoint's provenance. |
| `port_offset` | integer | no | `0` | Added to every port the services tree declares. |

Each entry in `sources`:

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `name` | string | yes | — | Unique per profile. Keys the cache and labels every status row and log line. |
| `https` | object | one of | absent | `url`, plus optional `auth_secret` and `verify`. |
| `oci` | object | one of | absent | `reference`, plus optional `auth_secret` and `verify`. |

Naming both `https` and `oci`, or neither, is a profile error. `auth_secret` names a key in the secrets document, never a value.

`verify` takes `sha256` (a 64-hex archive digest) and `ed25519_public_keys` (base64 32-byte keys). At least one is required for every source; an empty `verify` is refused. Verification is fatal at every step and runs in order: archive digest, manifest signature, per-file checksums, content hash.

Only the first source may be a base bundle. Every later source must own nothing but `marketplaces`, `plugins`, `skills`, `rules`, `hooks` and `artifacts`. Two sources claiming the same id, or the same base directory, is a boot error naming both.

### Managed MCP servers (`services/mcp/*.yaml`)

`crates/shared/models/src/mcp/deployment.rs`. Each file declares `mcp_servers.<name>` entries; the keys the bridge and boot validation depend on:

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `enabled` | bool | yes | — | Whether the server is served, validated and published to bridges. |
| `tool_policy` | enum `allow` \| `deny` \| `prompt` | yes, for every enabled server | — | The decision a bridge-managed client (Claude Code, Claude Desktop) applies to every tool the server exposes. `allow` skips the per-call prompt; `prompt` asks; `deny` blocks. A server without it is withheld from the signed bridge manifest and `admin config validate` / startup report `mcp_servers.<name>.tool_policy` as an error. |
| `endpoint` | string | for `external` servers | — | Absolute URL, or a path relative to the gateway. |
| `external_auth` | object | no | absent | Per-user bearer resolution for an `external` server (`token_endpoint`, `header`, `scheme`). |

## `extensions`

`crates/shared/manifest/src/profile/mod.rs:58`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `disabled` | list of string | no | `[]` | Extension IDs to disable. An ID in this list is excluded from registration. |

See [`concepts/extensions.md`](../concepts/extensions.md) for the extension model.

## Provider catalog and gateway — moved to the services tree

`providers:` and `gateway:` are not profile sections. A profile that still carries either
key fails to parse with `ProfileError::MovedToServices`, which names the key and the file it
belongs in (`crates/shared/manifest/src/profile/mod.rs`). The reason is what the two sections
are: a model catalog (providers, models, pricing, capabilities, limits) and the routes onto it
change only when the product changes, carry no credential, and are identical across every
environment — implementation configuration, like agents and MCP servers, not deployment
configuration; in a profile they would be copied by hand into every environment and drift.

Both live under `paths.services` and are loaded, merged, and validated by the services
loader (`crates/infra/loader/src/config_loader/`):

| Services key | Conventional file | Type | Meaning |
|--------------|-------------------|------|---------|
| `providers` | `services/ai/providers.yaml` | list of provider entries | The provider registry: each upstream's wire protocol, surface, endpoint, `api_key_secret`, extra headers, per-model pricing/capabilities/limits, and governance flags (`crates/shared/manifest/src/services/providers/`). Entries concatenate across includes; a provider name declared twice is a load error. |
| `gateway` | `services/ai/gateway.yaml` | object | The provider-facing inference proxy: `enabled`, `routes[]`, `default_provider`, `default_model`, `allow_unlisted_models`, `hidden_requires_route` (default `true`: a catalog model marked `hidden` is served only when a declared route pattern matches it, never through catalog membership or the route synthesised from `default_provider`), `require_scopes` (dimensions every request must resolve a scope value for; see [scope attribution](./http-api.md#scope-attribution)), `auth_scheme`, `inference_path_prefix`, `system_prompt_overrides[]`, `bridge_releases` (`crates/shared/manifest/src/services/gateway/`). First file to declare it wins across includes. |

Both files must be listed in the root aggregator's `includes:` (`services/config/config.yaml`);
`systemprompt admin setup` writes them from the embedded seed catalog when absent and appends
them. `admin config catalog …` and `admin config gateway …` edit those files, validate the
result against the merged registry, and never touch the profile.

### `gateway.routes[]`

`crates/shared/manifest/src/services/gateway/route.rs`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `id` | string | no | derived | Stable route id; synthesized from pattern and provider when empty. Addressed from `access_control_rules`. |
| `model_pattern` | string | yes | — | Match pattern. `*` matches all; `prefix*` and `*suffix` are supported; otherwise exact. |
| `provider` | string | yes | — | Provider name; must be declared in `providers`. The route carries no endpoint or credential of its own. |
| `upstream_model` | string | no | requested model | Override model name sent upstream. |
| `extra_headers` | map<string,string> | no | `{}` | Additional upstream request headers. May not name `authorization`, `x-api-key`, `x-goog-api-key`, `anthropic-version`, `content-type`, `content-length` or `host`. |
| `accepted_betas` | list<string> | no | all on first-party; none on Vertex AI | The `anthropic-beta` values forwarded from a client request. Vertex AI rejects a beta it does not serve, so a Vertex provider forwards only the betas listed here. The 1M context window needs no beta on either host. |
| `pricing` | object | no | absent | Route-level pricing override; otherwise the matching `providers[].models[].pricing` applies. |
| `when` | object | no | absent | Request-type match (`RouteMatch`). |
| `requires` | object | no | absent | Governance requirements the resolved model must satisfy (`RouteRequirements`). |
| `fallbacks` | list of `{provider, upstream_model}` | no | `[]` | Deployments tried in order after `provider` when a deployment exhausts its 429/503 retry budget, answers any other 5xx, or is unreachable. Each `provider` must be declared in `providers` and appear once per chain; each deployment is validated against the route's pricing and `requires` as the route it will serve. Each deployment sits behind its provider's circuit breaker (`resilience:`): tripped deployments are skipped while a healthy one remains, and when none is healthy all are tried in order. A provider (such as the Anthropic API) is only used when listed. The audit row records `served_provider` and `route_match` carries `failover:a->b->c`; cost is the serving deployment's catalog rate; `gateway_upstream_failovers_total{from,to,reason}` counts every hop. `upstream_model` defaults to the requested model. |
| `by_scope` | object | no | absent | Per-scope chains keyed by the value a request is attributed to in one scope dimension: `dimension` (a registered scope dimension, e.g. `project`), `chains` (`{<value>: {provider, upstream_model, fallbacks}}`, at least one, no empty key) and `unmapped` (`deny`, the default, or `shared`). A request with no value for the dimension uses the route's own chain; a mapped value uses its chain; an unmapped value is refused before any upstream call unless `unmapped: shared` sends it down the route's chain. `route_match` records `scope:<dimension>=<value>`. |
| `strategy` | `ordered` \| `weighted` \| `least_busy` | no | `ordered` | Which deployment of the chain a request tries first. `ordered` is the chain order. `weighted` draws the first attempt by `weight` among the deployments whose circuit breaker is closed and tries the rest in descending weight. `least_busy` sends the first attempt to the healthy deployment with the fewest requests in flight in this process (counted from send until the upstream answers; not cluster-wide). Failover after the first attempt is unchanged. Also settable per `by_scope` chain. A non-`ordered` strategy adds `strategy:<name>` to `route_match`; `gateway_deployment_selected_total{route,provider,strategy}` counts every selection. |
| `weight` | integer ≥ 1 | no | `1` | The primary deployment's weight under `strategy: weighted`; each `fallbacks` entry (and each `by_scope` chain and its fallbacks) takes its own `weight`. `weight: 0` is refused at load. A weight under `ordered` or `least_busy` is accepted and ignored. |
| `context_fallbacks` | list of `{provider, upstream_model}` | no | `[]` | Deployments for requests too large for the selected deployment's model. Before any upstream call the gateway estimates the request's input tokens from its text (system, messages, tool results; images and tool inputs are not counted, so the estimate errs low) and compares it with the model's catalog `limits.context_window`. A request that does not fit is sent to the `context_fallbacks` whose window fits it, in order (`route_match` records `context_window:a->b`; `gateway_context_fallbacks_total{from,to}` counts it); with none, it is refused with a 400 in the caller's wire format whose `error.error_key` is `context_window_exceeded`, naming the estimate, the limit and the model. A model with no declared window is never refused, failover deployments whose declared window is too small are skipped, and `max_tokens` is clamped to the model's output ceiling rather than refused. Each entry is validated like a `fallbacks` entry. Also settable per `by_scope` chain. |

Every provider endpoint is validated through the shared outbound-URL guard
(`validate_outbound_url`, `crates/shared/models/src/net/mod.rs`), which rejects loopback,
private-network, link-local and CGNAT destinations so the proxy cannot become an SSRF primitive.
`!include <file>` in a `system_prompt_overrides[].prompt` resolves relative to the file that
carries the `gateway:` block.

### Migrating a profile

Move the `providers:` block of `profile.yaml` into `services/ai/providers.yaml` and the
`gateway:` block into `services/ai/gateway.yaml`, list both under `includes:` in
`services/config/config.yaml`, and delete them from the profile. Nothing inside either block
changes shape. A profile that still carries a `gateway.catalog:` block (removed earlier) moves
its `providers[]` entries into `providers` and expresses each `models[]` entry as a
`gateway.routes[]` entry.

## `governance`

`crates/shared/manifest/src/profile/governance.rs`. Configures the authorization hook for the gateway and MCP planes. The hook is fail-closed: an absent `governance` block, absent `authz`, or unparseable config installs a deny-all hook.

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `authz` | object | no | absent | Authorization hook config. See [`governance.authz.hook`](#governanceauthzhook). |
| `quota.shared_subject_shards` | integer | no | `8` | Rows a quota window keyed by a shared subject (anything other than `user` or `api_key`) is spread over, 1–64. Admission checks the summed total; each extra shard can admit up to one more concurrent reservation past a ceiling. `1` restores exact single-row accounting. |
| `quota.principal_subject_shards` | integer | no | `1` | The same for `user` and `api_key` windows, 1–64. |

### `governance.authz.hook`

`crates/shared/manifest/src/profile/governance.rs:53`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `mode` | enum `webhook` \| `disabled` \| `unrestricted` | yes | — | `webhook`: POST each request to the server's own `/api/public/govern/authz` (derived from `server.host`/`server.port`); any transport error, non-2xx, or decode failure denies. `disabled`: deny every request (surface installed). `unrestricted`: allow every request — test/dev only. |
| `timeout_ms` | u64 | no | `500` | Per-request hook timeout. |
| `acknowledgement` | string | conditional | absent | Required for `unrestricted` mode; must equal the literal `I understand this disables all authorization` or bootstrap errors. |

```yaml
governance:
  authz:
    hook:
      mode: webhook
      timeout_ms: 500
```

## `observability`

`crates/shared/manifest/src/profile/observability.rs`. The gateway *ingests* OTLP at `POST /otel`; this block is the other direction. When `otlp` is set, the `otlp_export` scheduler job tails the audit tables and posts every completed AI request as a span — with a child span for each tool call in `tool_call_ledger` and each `governance_decisions` row under its trace — and every `logs` row as a log record, to the collector named here. Each request span carries the request's [scope attribution](./http-api.md#scope-attribution) as `systemprompt.scope.<dimension>` (the value) and `systemprompt.scope.<dimension>.source` (`header`, `api_key` or `default`), and the authenticating key as `systemprompt.api_key.id`, so spend can be grouped by any tenant dimension in the collector. Metrics are not exported; they stay on the Prometheus `/metrics` listener. Operating notes, including the `otlp_export_state` progress table: [operate.md §5](../guides/operate.md#5-ingest-and-export-opentelemetry-otlp).

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `otlp` | object | no | absent | The collector. Absent means nothing is exported and the job is a no-op. |
| `log_format` | enum `text` \| `json` | no | `text` | How the process writes its own log events to **stderr**. `json` writes one object per line (`timestamp`, `level`, `target`, `fields`, `spans`, `instance`) for Fluent Bit, Vector or Loki. Both formats redact the same secret-bearing fields. `runtime.output_format` governs CLI command output only, not this stream. |

### `observability.otlp`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `endpoint` | string | yes | — | Collector base URL, `http://` or `https://` with a host. OTLP/HTTP appends `/v1/traces` and `/v1/logs`; an endpoint that already ends in that path is used as given. `${VAR}` interpolation applies. |
| `protocol` | enum `http` \| `grpc` | no | `http` | `http` is OTLP/HTTP with protobuf bodies. `grpc` is reserved and **fails validation** as not yet supported — point the exporter at the collector's HTTP receiver (default port 4318). |
| `headers` | map string → string | no | `{}` | Sent verbatim on every export request (`Authorization`, `DD-API-KEY`, `X-Scope-OrgID`, …). Names must be valid HTTP header names; values may not contain line breaks. |
| `signals` | list of `traces` \| `logs` | no | `[traces, logs]` | Which signals to export. Must be non-empty with no repeats. |
| `batch_seconds` | u64 | no | `15` | Minimum seconds between two exports of the same signal (`1`–`3600`). The job's cron tick (default every 15 s, overridable in the scheduler config) is the upper bound on how often it runs; this is the lower bound on how often it ships. |

```yaml
observability:
  otlp:
    endpoint: https://otlp.example.com:4318
    headers:
      Authorization: "Bearer ${OTLP_TOKEN}"
    signals: [traces, logs]
    batch_seconds: 30
```

The job must also be enabled in the scheduler config (`services/scheduler/config.yaml`) under the name `otlp_export`; the profile block alone configures *where*, the scheduler entry configures *whether* and *how often*.

## `${VAR}` interpolation

Before YAML parsing, the loader substitutes `${VAR}` tokens in the raw profile text with the value of the environment variable `VAR` (`profile/mod.rs:79-86`). The pattern is exactly `${WORD}` (alphanumeric and underscore). An undefined variable is left literally in place (the `${VAR}` text is preserved, not blanked). Interpolation is textual and applies to any value in the document.

```yaml
server:
  external_url: ${PUBLIC_BASE_URL}
```

## Secrets envelope

Secret values are never stored in `profile.yaml`. They live in a separate JSON document referenced by the [`secrets`](#secrets) section, loaded by `SecretsBootstrap`. The envelope is customer-owned; the binary never holds a master key. Schema in `crates/shared/manifest/src/secrets.rs`.

| Field | JSON key | Type | Required | Notes |
|-------|----------|------|----------|-------|
| `oauth_at_rest_pepper` | `oauth_at_rest_pepper` | string | yes | Minimum 32 characters (`secrets.rs:13`); shorter values fail validation. |
| `database_url` | `database_url` | string | yes | Primary connection string. |
| `manifest_signing_secret_seed` | `manifest_signing_secret_seed` | string | yes | Standard base64 of exactly 32 random bytes; the Ed25519 manifest signing seed. Never generated at boot: every replica must carry the same value. Mint it with `systemprompt admin identity generate --json`. |
| `signing_key_pem` | `signing_key_pem` | string | yes on `cloud` and deployment hosts | Standard base64 of the full PKCS#8 PEM text (`-----BEGIN PRIVATE KEY-----` …) of the RSA JWT signing key. Local profiles may point `security.signing_key_path` at a file instead. Mint it with `systemprompt admin identity generate --json`. |
| `database_write_url` | `database_write_url` | string | no | Separate write endpoint. |
| `database_session_url` | `database_session_url` | string | no | Direct (unpooled) primary connection for `LISTEN`, session advisory locks and migrations; set it when `database_url` goes through a transaction-mode pooler. See [guides/database-connections.md](../guides/database-connections.md). |
| `external_database_url` | `external_database_url` | string | no | Used when `database.external_db_access` is `true`. |
| `internal_database_url` | `internal_database_url` | string | no | Internal connection string. |
| `gemini` | `gemini` | string | no | Gemini API key. |
| `anthropic` | `anthropic` | string | no | Anthropic API key. |
| `openai` | `openai` | string | no | OpenAI API key. |
| `github` | `github` | string | no | GitHub token. |
| `moonshot` | `moonshot` | string | no | Moonshot/Kimi key. |
| `qwen` | `qwen` | string | no | Qwen/DashScope key. |
| `encryption_master_key` | `encryption_master_key` | string | yes | 32 random bytes as 64 hex characters (`openssl rand -hex 32`); `admin setup` and `admin identity generate` mint it. Encrypts the gateway's per-request accounting receipts (and extensions' stored grants), and seals the bearer token the MCP proxy caches per session in `mcp_proxy_identities`. Validated at secrets bootstrap: a missing or malformed key is refused before migrations run, whether or not the gateway is enabled. Preserve it across restarts and replicas. |
| (custom) | any other key | string | no | Extra keys flatten into a `custom` map and are addressable by name. |

`null` values are stripped before deserialization (`secrets.rs:62`).

### Environment-variable source

When `secrets.source` is `env` (or a Fly.io container is detected via `FLY_APP_NAME`), secrets are read from environment variables instead of the file (`secrets/loader.rs`). Mapping:

| Secret | Environment variable |
|--------|----------------------|
| `oauth_at_rest_pepper` | `OAUTH_AT_REST_PEPPER` (required) |
| `database_url` | `DATABASE_URL` (required) |
| `manifest_signing_secret_seed` | `MANIFEST_SIGNING_SECRET_SEED` (required) |
| `signing_key_pem` | `SIGNING_KEY_PEM` (required on deployment hosts) |
| `database_write_url` | `DATABASE_WRITE_URL` |
| `database_session_url` | `DATABASE_SESSION_URL` |
| `external_database_url` | `EXTERNAL_DATABASE_URL` |
| `internal_database_url` | `INTERNAL_DATABASE_URL` |
| `gemini` | `GEMINI_API_KEY` |
| `anthropic` | `ANTHROPIC_API_KEY` |
| `openai` | `OPENAI_API_KEY` |
| `github` | `GITHUB_TOKEN` |
| `moonshot` | `MOONSHOT_API_KEY` or `KIMI_API_KEY` |
| `qwen` | `QWEN_API_KEY` or `DASHSCOPE_API_KEY` |
| `encryption_master_key` | `ENCRYPTION_MASTER_KEY`, listed in `SYSTEMPROMPT_CUSTOM_SECRETS` (required) |
| (custom) | names listed in `SYSTEMPROMPT_CUSTOM_SECRETS` (comma-separated) |

## Bootstrap order

Configuration is assembled in a fixed sequence; later stages depend on earlier ones (`crates/infra/config/src/bootstrap/`). The type-state `BootstrapSequence` enforces that secrets cannot initialize before the profile (`bootstrap/mod.rs:48-93`).

1. **ProfileBootstrap** — reads `profile.yaml`, performs `${VAR}` interpolation, resolves relative paths, deserializes into `Profile`.
2. **ServicesState** — resolves the services root first: with `services.sources` configured, each bundle is fetched, verified, composed and swapped in before paths are derived; otherwise the root is `paths.services`. It then loads the tree through the services loader (`crates/infra/loader/src/services_state.rs`): merges every include, resolves the gateway spec to its runtime form, validates the provider registry and the gateway's references into it. A tree that does not load is a boot failure.
3. **SecretsBootstrap** — loads the secrets document from the resolved source (file, environment, or Vault KV v2), validates required fields (e.g. the 32-char pepper), ensures the manifest signing seed. A Vault fetch that fails aborts the boot under every validation mode.
4. **CredentialsBootstrap** — loads cloud credentials (file, or environment in a Fly.io container) and validates them against the cloud API for `cloud` targets (`crates/infra/cloud/src/credentials_bootstrap/`).
5. **Config** — composes the validated profile and resolved secrets into the runtime config.
6. **AppContext** — constructs the application context (database pool, services, resolved system admin) consumed by the API and CLI.

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
  external_url: http://127.0.0.1:8080
paths:
  root: ../../..
  system: .
  services: services
  bin: target/release
security:
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
  "database_url": "postgresql://user:pass@localhost:5432/db",
  "manifest_signing_secret_seed": "<base64-encoded 32-byte seed>",
  "encryption_master_key": "<64 hex characters>"
}
```

## Related

- [HTTP API reference](./http-api.md) — routes whose rate limits are configured above.
- [Extensions](../concepts/extensions.md) — what the `extensions.disabled` list controls.
- [Feature flags](./feature-flags.md) — facade build features that bring in the API, CLI, and gateway code paths.
