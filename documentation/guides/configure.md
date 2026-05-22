# Configure a profile

How to write and manage the `profile.yaml` that drives a systemprompt deployment. This guide is task-oriented; for the exhaustive key-by-key schema see [../reference/configuration.md](../reference/configuration.md).

A profile is the single source of truth for one deployment. There are no environment-variable fallbacks for the profile itself — every setting the binary needs is resolved from the profile and its referenced secrets file. The binary reads a profile directory under `.systemprompt/profiles/<name>/`, with `profile.yaml` at its root.

## Prerequisites

- A running PostgreSQL 18+ instance and a connection URL for it.
- A directory the service account can read: `.systemprompt/profiles/<name>/`.
- A secrets file (JSON) the service account can read, holding at minimum `oauth_at_rest_pepper` and `database_url`.

## 1. Choose a profile name and environment

Each profile has a `name` and a `runtime.environment`. The environment is one of `development`, `test`, `staging`, `production` (`crates/shared/models/src/profile/runtime.rs:41`). Set it to `production` for production deployments; it gates development-only conveniences.

```yaml
name: production
runtime:
  environment: production
  log_level: normal        # quiet | normal | verbose | debug
  output_format: json      # use json for machine-ingestable logs
```

`log_level` maps to a tracing filter: `quiet`→`error`, `normal`→`info`, `verbose`→`debug`, `debug`→`trace` (`runtime.rs:104`).

## 2. Set the server section

The `server` section binds the listener and declares the URLs the binary advertises in discovery and OAuth metadata.

```yaml
server:
  host: 0.0.0.0
  port: 8080
  api_server_url: https://api.example.com
  api_internal_url: http://systemprompt:8080
  api_external_url: https://api.example.com
  use_https: false                    # TLS is terminated at the reverse proxy
  cors_allowed_origins:
    - https://app.example.com
  trusted_proxies:
    - 10.0.0.0/8                       # CIDRs whose X-Forwarded-For is trusted
  max_concurrent_streams: 1024         # per-replica cap on A2A SSE streams
```

`trusted_proxies` is required if a reverse proxy sets `X-Forwarded-For`, `X-Real-IP`, or `CF-Connecting-IP`. With an empty list the platform treats every connection as direct and ignores those headers (`crates/shared/models/src/profile/server.rs`). Behind a proxy, list the proxy's CIDR or client IPs are not attributed correctly.

## 3. Set the database section

`database` declares the engine type. The connection URL is **not** in the profile — it is a secret (see step 5).

```yaml
database:
  type: postgres
  external_db_access: false
```

The DB connection string is read from `database_url` in the secrets file (`crates/shared/models/src/secrets.rs:22`). Optional `database_write_url`, `external_database_url`, and `internal_database_url` secrets support split read/write or internal/external endpoints.

## 4. Set the paths section

`paths` tells the binary where its system root, services tree, and binary live. Relative paths resolve against the profile directory.

```yaml
paths:
  system: /var/lib/systemprompt
  services: /var/lib/systemprompt/services
  bin: /usr/local/bin
  web_path: /var/lib/systemprompt/web/dist   # optional; only if serving a static site
  storage: /var/lib/systemprompt/storage     # optional
  geoip_database: /var/lib/systemprompt/GeoLite2-City.mmdb  # optional
```

`system`, `services`, and `bin` are required (`crates/shared/models/src/profile/paths.rs`). Omit `web_path` for a headless API-only deployment.

## 5. Wire secrets

The deployment model is customer-owned: the binary never holds the master key and performs no symmetric at-rest encryption of the secrets file. Your key-management tooling opens the envelope and presents plaintext to the binary, either as a file or as environment variables.

Point the profile at the secrets source:

```yaml
secrets:
  source: file              # file | env
  secrets_path: ../../secrets/production.secrets.json
  validation: strict        # strict | warn (default) | skip
```

With `source: file`, `secrets_path` is the path to the JSON document. With `source: env`, the same keys are read from the process environment (`crates/shared/models/src/profile/secrets.rs`).

The minimum secrets document:

```json
{
  "oauth_at_rest_pepper": "<>= 32-character random string>",
  "database_url": "postgresql://systemprompt:<pw>@db.internal:5432/systemprompt",
  "manifest_signing_secret_seed": "<base64-encoded 32-byte seed>",
  "anthropic": "sk-ant-...",
  "openai": "sk-..."
}
```

`oauth_at_rest_pepper` must be at least 32 characters (`crates/shared/models/src/secrets.rs:13`). `manifest_signing_secret_seed` is a base64-encoded 32-byte Ed25519 seed; if absent and the secrets path is writable, `systemprompt admin bridge rotate-signing-key` generates one. Provider keys (`anthropic`, `openai`, `gemini`, `github`, `moonshot`, `qwen`) are optional; add only those you use. Any additional key/value pairs are accepted and exposed as custom secrets.

Set `0600` permissions on a plain JSON secrets file and own it with the service account. Never commit it to git.

### `${VAR}` interpolation

Any string value in `profile.yaml` may reference an environment variable as `${VAR}`; it is substituted at load time (`crates/shared/models/src/profile/mod.rs`). This keeps host-specific values out of the committed profile:

```yaml
server:
  api_external_url: ${API_EXTERNAL_URL}
secrets:
  secrets_path: ${SECRETS_PATH}
```

## 6. Set the security section

`security` configures the JWT plane and OAuth issuer. The signing key is RSA (the JWT plane is RS256-only).

```yaml
security:
  jwt_issuer: https://api.example.com
  jwt_access_token_expiration: 900       # seconds
  jwt_refresh_token_expiration: 2592000  # seconds
  jwt_audiences:
    - api
  signing_key_path: signing_key.pem      # relative to the profile dir
  allow_registration: false              # disable open registration in production
  trusted_issuers:                        # optional RFC 8693 federation
    - issuer: https://idp.example.com
      jwks_uri: https://idp.example.com/.well-known/jwks.json
      audience: api
```

Generate the signing key with `systemprompt admin keys generate` (RSA-2048). `trusted_issuers` lets the platform accept subject tokens from external identity providers, verified against each issuer's published JWKS (`crates/shared/models/src/profile/security.rs`).

## 7. Enable governance and the authorization hook

Authorization is fail-closed. If the `governance` block is absent, the `authz` block is absent, or the config is unparseable, the platform installs a deny-all hook — misconfiguration never silently grants access (`crates/shared/models/src/profile/governance.rs`).

For production, point the authz hook at your policy webhook:

```yaml
governance:
  authz:
    hook:
      mode: webhook
      url: https://policy.example.com/govern/authz
      timeout_ms: 500
```

In `webhook` mode the platform POSTs every governed request to `url`; any transport error, non-2xx, or decode failure denies the request. The three modes are:

| Mode | Behaviour |
|------|-----------|
| `webhook` | Production. Each request is authorized by the external hook; fail-closed on error. |
| `disabled` | Installs a deny-all hook. Use to keep the surface installed while authz is inactive. |
| `unrestricted` | Test/dev only. Allows every request. Requires `acknowledgement: "I understand this disables all authorization"` or bootstrap errors. |

Never use `unrestricted` in production.

## 8. Validate the profile

Bootstrap runs in a fixed order: ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config → AppContext (`crates/infra/config/src/bootstrap/`). Each stage validates its inputs and fails fast. Starting the server (`systemprompt infra services serve`) or running `systemprompt infra db status` exercises the profile and secrets load; a missing `database_url` or `oauth_at_rest_pepper` fails with an explicit error.

## Next steps

- [reference/configuration.md](../reference/configuration.md) — the complete profile schema.
- [deploy-production.md](deploy-production.md) — production topology, HA, backup, and rotation.
- [operate.md](operate.md) — day-2 health checks, metrics, and troubleshooting.
