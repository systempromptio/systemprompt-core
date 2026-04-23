# Threat Model

STRIDE-style threat model for the systemprompt.io governance binary. Scope is the code in this repository plus the surrounding operational context of a typical self-hosted deployment. It does not cover threats against the customer's upstream AI providers or downstream consumers — those sit outside our trust boundary by design.

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
│   audience + issuer checks, RBAC             │
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
│   crates/shared/models (secrets bootstrap),  │
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
| TB3 | Secrets boundary | ciphertext on disk when customer uses envelope encryption (KMS / Vault / sops); plaintext only in the binary's process memory after launch | the master key that opens the envelope — owned by the customer's key-management programme, never held by the binary |
| TB4 | Audit boundary | append-only structured event stream | downstream SIEM / object store |
| TB5 | Provider egress | customer-controlled outbound HTTPS to providers | provider cloud |

## 3. Assets

1. **Bearer credentials** — JWTs, OAuth tokens, refresh tokens. Short-lived; transient in memory during request handling.
2. **Provider API keys and JWT signing secret** — Anthropic, OpenAI, Gemini keys, JWT HMAC secret. Loaded from a profile-referenced secrets file or environment variables into process memory for the lifetime of the process.
3. **MCP allowlist configuration** — signed manifest of permitted tool servers (integrity is load-bearing).
4. **Audit and log events** — record of every governed interaction (integrity and non-repudiation are the product).
5. **User / RBAC data** — who can call what, scoped by handler boundary.
6. **Prompt and response content** — may contain PHI, source code, or other regulated data in-flight (never persisted unless customer opts in).

## 4. STRIDE Analysis

Each threat is mapped to the component where it originates, the mitigation in code, and residual risk that remains after mitigation.

### 4.1 Spoofing

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Forged JWT impersonating a user or service | `crates/infra/security/src/auth/validation.rs` | `jsonwebtoken` crate with `Validation::new(Algorithm::HS256)` — only HS256 accepted; `alg: none` and asymmetric-algorithm confusion attacks rejected by library default. `set_issuer` + `set_audience` enforced. Current JWT implementation uses an HMAC-SHA256 shared secret; RS256/ES256/EdDSA asymmetric verification is on the roadmap for direct customer-IdP federation | Shared-secret model means JWT signing key must be protected at the same level as the service itself. Customer IdPs that sign with asymmetric keys are currently integrated via OAuth2 code flow, not direct JWT verification. |
| OAuth authorisation code interception | `crates/domain/oauth/src/repository/oauth/auth_code.rs` | PKCE `S256` verified (SHA-256 of code_verifier compared to stored code_challenge); PKCE method `plain` explicitly rejected; codes are single-use (`used_at` enforced), short-TTL (`expires_at`), and `redirect_uri` must match the original request | Cannot prevent compromise of the authorising IdP itself. |
| Provider response substitution | `crates/domain/ai` | TLS via the customer's trust store; response content-type and schema validation | Upstream provider compromise is outside our trust boundary. |
| MCP server impersonation | `crates/domain/mcp`, `crates/infra/security/src/manifest_signing.rs` | Signed manifest allowlist; manifest signature verified at load using the deployment's JWT HMAC secret (shared with the signing tool) | Manifest-signing key compromise rotates via config reload. |

### 4.2 Tampering

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| SQL injection | `crates/infra/database`, all repositories | 100% of request-path queries use `sqlx::query!`, `sqlx::query_as!`, `sqlx::query_scalar!` — compile-time verified against the live schema. Zero `sqlx::query_unchecked!`. A small number of `format!`-constructed DDL statements exist in admin/setup CLI paths (`crates/entry/cli/src/commands/admin/setup/*.rs`) where parameters are drawn from the operator's config file, not user input | Verified by repository audit. |
| Audit / log tampering in-flight | `crates/infra/logging`, `crates/domain/analytics` | Events written synchronously within the request transaction. Append-only discipline is enforced by DB role least-privilege: the systemprompt role holds `INSERT, SELECT` on audit/log tables and does not hold `UPDATE, DELETE` — see deployment guide §3. No schema-level immutability triggers are shipped; customers whose compliance programme requires defense-in-depth may add a BEFORE UPDATE/DELETE trigger (recommended DDL in the deployment guide) | Post-insertion DB admin compromise not addressed at the schema level — layered defense via role grants + recommended trigger. |
| MCP allowlist tampering | `crates/domain/mcp`, `crates/infra/security/src/manifest_signing.rs` | Manifest loaded from signed source; HMAC-SHA256 signature verified at load using the deployment JWT secret; hot reload re-verifies | Requires key management discipline on customer side. |
| Prompt/response modification in transit | entry | TLS 1.2+ required (customer-configured certificate); no plaintext HTTP listener | TLS downgrade protected by customer reverse proxy config. |

### 4.3 Repudiation

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| User denies having issued a governed request | `crates/infra/events` | Every request is bound at entry to an authenticated identity; audit row carries JWT `sub`, request ID, timestamp, full rule-evaluation trace | Relies on customer's IdP logs for the identity-to-human binding. |
| Tool invocation cannot be attributed | `crates/domain/mcp`, `crates/domain/agent` | A2A `Task.contextId` + `Message.messageId` propagate through the call graph; audit events reference these IDs | Customer must preserve audit retention for forensic window. |

### 4.4 Information Disclosure

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Secrets leaked via logs or error traces | `crates/infra/logging` | Structured tracing only logs typed error values (`%e`), not raw secret inputs; bearer tokens are stripped before request logging; auditor review of `crates/infra/security/src/auth/extraction.rs` and `validation.rs` confirms no secret literal reaches the tracing macros | Third-party crate log output still possible; customer filters at the log shipper if required. |
| Provider API keys and JWT secret exposed at rest | `crates/shared/models/src/secrets_bootstrap.rs` | The binary itself does **not** perform symmetric at-rest encryption of secrets. Secrets are loaded from a profile-referenced JSON file or environment variables into process memory at startup. **The expected deployment model is that the customer uses their existing envelope-encryption infrastructure** (HashiCorp Vault, AWS/GCP/Azure KMS, sops + age, or equivalent) to protect the secrets file on disk and decrypt it into the binary's environment or into a tmpfs-mounted file at launch. The master key never enters the binary — an architecturally stronger position than a binary-held AEAD key would be, because the customer's existing key-management programme governs it end-to-end. Deployment guide §2 documents the supported patterns | Filesystem permissions on the profile secrets file (0600, dedicated service account) are the fallback for deployments that choose not to use envelope encryption — acceptable only outside regulated contexts. |
| Audit data exfiltration via DB compromise | `crates/infra/database` | DB role for systemprompt is least-privilege (`SELECT, INSERT` on log and analytics_events tables; no `UPDATE`, `DELETE`). Deployment guide recommends a separate read-only role for SIEM export | Database-admin-level compromise remains a high-impact event — mitigated operationally (customer RBAC on Postgres), not architecturally. Customers can layer a BEFORE UPDATE/DELETE trigger on audit tables for defense-in-depth (recommended DDL in deployment guide §4). |
| PHI / regulated content accidentally persisted | `crates/domain/ai` | Prompt and response bodies are not persisted by default; opt-in retention is config-gated | If a customer enables retention, they inherit responsibility for its lifecycle. |
| Cross-tenant data bleed | `crates/domain/users`, handler boundary | Handler-boundary RBAC enforced in `crates/domain/mcp/src/middleware/rbac.rs`; repository queries scope by tenant where applicable; integration tests enforce scoping | Depends on correct handler wiring — covered by the test suite. |

### 4.5 Denial of Service

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Request flood at entry | `crates/entry/api` | Axum tower limits (concurrency, timeout, body size); customer typically front-loads with WAF / LB | High-volume DoS mitigation is operational; binary handles ~200 concurrent governance requests sub-5ms p50 in-process. |
| Expensive rule evaluation | `crates/app/runtime` | Rule evaluator has bounded complexity; no user-supplied regex/eval | Complex allowlist configurations increase p99 linearly — benchmark before large rollouts. |
| Upstream provider slowness propagating back pressure | `crates/domain/ai` | Timeouts per-provider, configurable; circuit-breaker pattern on repeated failures | Governance correctness preferred over availability — we fail closed. |
| Audit write pressure | `crates/infra/events` | Batched writes where transactionally safe; async forwarding to SIEM; slow SIEM cannot block request path | Postgres write saturation remains customer-sizeable. |

### 4.6 Elevation of Privilege

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| User escalating scope via token manipulation | `crates/infra/security` | Scopes derived from JWT claims at entry; immutable through the request lifecycle; handler-boundary RBAC re-checks at each crossing | Relies on IdP claim correctness. |
| Extension gaining undeclared capabilities | `crates/shared/extension` | Extensions register via `inventory` at compile time; trait surface is typed and narrow; no runtime codeload | Supply chain of compiled-in extensions is the customer's build-time decision. |
| MCP tool invoking outside declared surface | `crates/domain/mcp` | Allowlist gates every tool call; declared capabilities enforced at call time; server manifest pins transport and methods | Tool servers themselves are trusted by the allowlist; customers control what they list. |
| Privilege held in shared state | All layers | No global mutable singletons; AppContext is explicit and passed through; Shared layer enforces "no state" architectural rule | Enforced by layer discipline and reviewed on every PR. |

## 5. Assumptions and Non-Goals

- The customer's OS, network, and hypervisor are trusted. We do not defend against a malicious kernel.
- The customer's OAuth Identity Provider is trusted. We verify signatures but cannot detect IdP compromise.
- The customer's Postgres instance is trusted for confidentiality and integrity. Database-level encryption at rest is a customer control.
- The customer's upstream AI providers (Anthropic, OpenAI, etc.) are trusted to the extent of their SLAs. We govern what leaves the binary; we do not attest what the provider does with it.
- Physical security of host infrastructure is out of scope.

## 6. Validation and Review

- Threat model is reviewed on every release that touches the security, events, or entry crates.
- Tabletop review scheduled quarterly; findings roll into the next minor release.
- External penetration testing available to enterprise customers under commercial agreement; customer-commissioned tests are welcomed with reasonable coordination.

## 7. Change Log

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
