# Threat Model

STRIDE-style threat model for the systemprompt.io governance binary. Scope is the code in this repository plus the surrounding operational context of a typical self-hosted deployment. It does not cover threats against the customer's upstream AI providers or downstream consumers — those sit outside the trust boundary by design.

## 1. System Overview

systemprompt.io is a single Rust binary that terminates AI traffic, applies governance rules, and emits structured audit events. It runs inside the customer's network. PostgreSQL is its only required external dependency.

High-level data flow:

```
Developer / Agent
    │ HTTPS (Bearer JWT)
    ▼
┌──────────────────────────────────────────────┐
│ Entry Layer                                  │
│   crates/entry/api                           │
│   TLS termination, routing, /v1/messages     │
└──────────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────────┐
│ Security / Identity                          │
│   crates/infra/security                      │
│   OAuth2/OIDC w/ PKCE, JWT verification,     │
│   issuer checks, RBAC                        │
└──────────────────────────────────────────────┘
    │
    ▼
┌──────────────────────────────────────────────┐
│ Governance Pipeline (App / Domain)           │
│   crates/app/runtime, crates/domain/ai,      │
│   crates/domain/mcp, crates/domain/agent     │
│   Rule eval, tool allowlist, MCP server      │
│   policy, provider selection                 │
└──────────────────────────────────────────────┘
    │                                   │
    │ egress: provider API              │ internal: tool / MCP
    ▼                                   ▼
┌──────────────────────────────────────────────┐
│ Audit + Secrets + Persistence                │
│   crates/infra/logging (structured audit),   │
│   crates/infra/config (secrets bootstrap),   │
│   crates/infra/database (sqlx + Postgres)    │
└──────────────────────────────────────────────┘
    │
    ▼
PostgreSQL (customer-managed)
```

Architectural detail: see the crate layout described in the repository-root `README.md` and `CLAUDE.md`.

## 2. Trust Boundaries

| # | Boundary | Inside | Outside |
|---|----------|--------|---------|
| TB1 | Network edge | customer VPC / cluster | developers, agents, CI jobs |
| TB2 | Process boundary | systemprompt binary | OS, Postgres, provider SDKs |
| TB3 | Secrets boundary | ciphertext on disk when the customer uses envelope encryption (KMS / Vault / sops); plaintext only in the binary's process memory after launch | the master key that opens the envelope — owned by the customer's key-management programme, never held by the binary |
| TB4 | Audit boundary | append-only structured event stream | downstream SIEM / object store |
| TB5 | Provider egress | customer-controlled outbound HTTPS to providers | provider cloud |

## 3. Assets

1. **Bearer credentials** — JWTs, OAuth tokens, refresh tokens. Short-lived; transient in memory during request handling.
2. **Provider API keys and the JWT signing key** — Anthropic, OpenAI, Gemini keys, plus the RSA private key that signs first-party tokens. Loaded from a profile-referenced secrets file or environment into process memory for the lifetime of the process.
3. **MCP allowlist configuration** — signed manifest of permitted tool servers (integrity is load-bearing).
4. **Audit and log events** — record of every governed interaction (integrity and non-repudiation are the product).
5. **User / RBAC data** — who can call what, scoped by handler boundary.
6. **Prompt and response content** — may contain PHI, source code, or other regulated data in-flight (never persisted unless the customer opts in).

## 4. STRIDE Analysis

Each threat is mapped to the component where it originates, the mitigation in code, and residual risk that remains after mitigation.

### 4.1 Spoofing

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Forged JWT impersonating a user or service | `crates/infra/security/src/auth/validation.rs` | The binary builds `Validation::new(Algorithm::RS256)` and rejects any header whose `alg` is not `RS256` (`validation.rs:92`); the JWT plane is RS256-only — there is no ES256/ES384/EdDSA acceptance path. The active `kid` in the header is resolved against an in-process `TokenAuthority` cache loaded from the deployment's signing key and the JWKS sets fetched for every entry in `profile.security.trusted_issuers`. HS256, `alg: none`, and algorithm-confusion attacks are all rejected. `set_issuer` is enforced; `exp`/`nbf`/`iat` are checked with a pinned 30s leeway and the `act` delegation chain is depth-capped. Tokens without a `kid`, or signed under an unknown key, fail validation. | Trust in the issuer's private-key custody. Customer IdPs that publish a JWKS can be federated by adding their issuer URL to `trusted_issuers`. Audience claims are not yet validated at the primary API extractor (`validate_aud = false`); per-surface audience isolation is a tracked open item. |
| OAuth authorisation code interception | `crates/domain/oauth/src/repository/oauth/auth_code.rs` | PKCE `S256` verified — SHA-256 of the code verifier compared constant-time (`ct_eq`) to the stored challenge (`auth_code.rs:238-275`); PKCE method `plain` explicitly rejected; codes are single-use (`used_at` enforced), short-TTL (`expires_at`), and the `redirect_uri` must match the original request. Replay of a used code revokes the entire refresh-token family. | A challenge is verified only when one was stored at authorize time; PKCE is not yet mandated server-side for public clients (tracked open item). Compromise of the authorising IdP itself cannot be prevented. |
| Provider response substitution | `crates/domain/ai` | TLS via the customer's trust store; response content-type and schema validation | Upstream provider compromise is outside the trust boundary. |
| MCP server impersonation | `crates/domain/mcp`, `crates/infra/security/src/manifest_signing.rs` | Signed manifest allowlist; manifest signature verified at load using the deployment's Ed25519 key (`ed25519_dalek`) over canonical JSON. The `manifest_signing_secret_seed` is distinct from the OAuth signing key. | Manifest-signing key compromise rotates via `systemprompt admin bridge rotate-signing-key`. |

### 4.2 Tampering

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| SQL injection | `crates/infra/database`, all repositories | Request-path queries use `sqlx::query!`, `sqlx::query_as!`, `sqlx::query_scalar!` — compile-time verified against the live schema. `format!`-constructed DDL exists only in admin/setup CLI paths (`crates/entry/cli/src/commands/admin/setup/*.rs`) where parameters are drawn from the operator's config file, not user input. | Verified by repository audit. |
| Audit / log tampering in-flight | `crates/infra/logging` | Events written synchronously within the request transaction. Append-only discipline is an operator-provisioned control: the systemprompt DB role is granted `INSERT, SELECT` (not `UPDATE, DELETE`) on the audit/log tables. **No `GRANT` statements ship in the schema migrations** — the grant is applied by the operator per the deployment guide. No schema-level immutability triggers are shipped; customers requiring defense-in-depth may add a BEFORE UPDATE/DELETE trigger (recommended DDL in the deployment guide). | Post-insertion DB-admin compromise is not addressed at the schema level — layered defense via role grants plus the optional trigger. |
| MCP allowlist tampering | `crates/domain/mcp`, `crates/infra/security/src/manifest_signing.rs` | Manifest loaded from signed source; Ed25519 signature verified at load using the deployment's `manifest_signing_secret_seed`; hot reload re-verifies. | Requires key-management discipline on the customer side. |
| Prompt/response modification in transit | entry | TLS 1.2+ required (customer-configured certificate); no plaintext HTTP listener. | TLS downgrade protected by customer reverse-proxy config. |

### 4.3 Repudiation

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| User denies having issued a governed request | `crates/infra/events` | Every request is bound at entry to an authenticated identity; the audit row carries the JWT `sub`, request ID, timestamp, and full rule-evaluation trace. | Relies on the customer's IdP logs for the identity-to-human binding. |
| Tool invocation cannot be attributed | `crates/domain/mcp`, `crates/domain/agent` | A2A (agent-to-agent) `Task.contextId` + `Message.messageId` propagate through the call graph; audit events reference these IDs. | Customer must preserve audit retention for the forensic window. |

### 4.4 Information Disclosure

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Secrets leaked via logs or error traces | `crates/infra/logging` | Structured tracing logs typed error values, not raw secret inputs; bearer tokens are stripped before request logging. | Third-party crate log output remains possible; the customer filters at the log shipper if required. |
| Provider API keys and JWT signing key exposed at rest | `crates/infra/config/src/bootstrap/secrets/`, `crates/shared/models/src/secrets.rs` | The binary does **not** perform symmetric at-rest encryption of secrets. Secrets are loaded from a profile-referenced JSON file or environment into process memory at startup. The expected deployment model is that the customer uses their existing envelope-encryption infrastructure (HashiCorp Vault, AWS/GCP/Azure KMS, sops + age, or equivalent) to protect the secrets file on disk and decrypt it into the binary's environment or a tmpfs-mounted file at launch. The master key never enters the binary — the customer's key-management programme governs it end-to-end. The deployment guide documents the supported patterns. | Filesystem permissions on the secrets file (0600, dedicated service account) are the fallback for deployments that do not use envelope encryption — acceptable only outside regulated contexts. |
| Audit data exfiltration via DB compromise | `crates/infra/database` | The systemprompt DB role is provisioned least-privilege (`INSERT, SELECT` on the `logs` and `analytics_events` tables; no `UPDATE`, `DELETE`); the deployment guide recommends a separate read-only role for SIEM export. The grant is operator-provisioned, not shipped in migrations. | Database-admin-level compromise remains high-impact — mitigated operationally (customer Postgres RBAC), not architecturally. A BEFORE UPDATE/DELETE trigger may be layered for defense-in-depth. |
| PHI / regulated content accidentally persisted | `crates/domain/ai` | Prompt and response bodies are not persisted by default; opt-in retention is config-gated. | If a customer enables retention, they inherit responsibility for its lifecycle. |
| Cross-tenant data bleed | `crates/domain/mcp/src/middleware/rbac.rs`, handler boundary | Handler-boundary RBAC enforced in `crates/domain/mcp/src/middleware/rbac.rs`; repository queries scope by tenant where applicable. | Depends on correct handler wiring. Runtime data-tenancy at the application layer is narrow — read tenant-scoping claims accordingly. |

### 4.5 Denial of Service

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Request flood at entry | `crates/entry/api` | A request body-size limit is wired at the API edge: `DefaultBodyLimit::max(2 MiB)` (`crates/entry/api/src/services/server/builder.rs:93`). IP-keyed rate limiting and bot/ban controls are applied as middleware behind the trusted-proxy client-IP resolver. No global tower `ConcurrencyLimit` or `TimeoutLayer` is wired in the builder; concurrency and timeout bounding is expected at the customer's reverse proxy / load balancer. | High-volume DoS mitigation is operational; the customer typically front-loads with a WAF / LB. |
| Expensive rule evaluation | `crates/app/runtime` | Rule evaluator has bounded complexity; no user-supplied regex/eval. | Complex allowlist configurations increase p99 linearly — benchmark before large rollouts. |
| Upstream provider slowness propagating back pressure | `crates/domain/ai` | Every provider is wrapped in a timeout/retry/circuit-breaker/bulkhead decorator (`crates/domain/ai/.../provider_factory.rs`); timeouts are per-provider and configurable. | Governance correctness is preferred over availability — the binary fails closed. |
| Audit write pressure | `crates/infra/events` | Batched writes where transactionally safe; async forwarding to SIEM so a slow SIEM cannot block the request path. | Postgres write saturation remains customer-sizeable. |

### 4.6 Elevation of Privilege

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| User escalating scope via token manipulation | `crates/infra/security` | Scopes derive from JWT claims at entry and are immutable through the request lifecycle; `user_type` is re-derived from the permission set and a disagreeing claim is rejected; handler-boundary RBAC re-checks at each crossing. | Relies on IdP claim correctness. |
| Extension gaining undeclared capabilities | `crates/shared/extension` | Extensions register via `inventory` at compile time; the trait surface is typed and narrow; no runtime code load. | The supply chain of compiled-in extensions is the customer's build-time decision. |
| MCP tool invoking outside declared surface | `crates/domain/mcp` | The allowlist gates every tool call; declared capabilities are enforced at call time; the server manifest pins transport and methods. | Tool servers themselves are trusted by the allowlist; customers control what they list. |
| Privilege held in shared state | All layers | No global mutable singletons; AppContext is explicit and passed through; the shared layer enforces the "no state" architectural rule. | Enforced by layer discipline and reviewed on every PR. |

## 5. Assumptions and Non-Goals

- The customer's OS, network, and hypervisor are trusted. The binary does not defend against a malicious kernel.
- The customer's OAuth Identity Provider is trusted. Signatures are verified but IdP compromise cannot be detected.
- The customer's Postgres instance is trusted for confidentiality and integrity. Database-level encryption at rest is a customer control.
- The customer's upstream AI providers are trusted to the extent of their SLAs. The binary governs what leaves it; it does not attest what the provider does with it.
- Physical security of host infrastructure is out of scope.

## 6. Validation and Review

- The threat model is reviewed on every release that touches the security, events, or entry crates.
- Tabletop review is scheduled quarterly; findings roll into the next minor release.
- External penetration testing is available to enterprise customers under commercial agreement; customer-commissioned tests are welcomed with reasonable coordination.

## 7. Change Log

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
| 2026-05-22 | Corrected DoS row to the body-size limit actually wired (`DefaultBodyLimit::max(2 MiB)`); removed unverified concurrency/timeout-layer claim. Confirmed RS256-only JWT plane and the RBAC path. Repointed secrets-bootstrap citations to `crates/infra/config/src/bootstrap/secrets/`. Marked audit-table grant as operator-provisioned, and `validate_aud=false` / PKCE-mandate as tracked open items. |
