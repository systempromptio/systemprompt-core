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
│   routing; multi-dialect inference surface:   │
│   /v1/messages, /v1/responses,                │
│   /v1/chat/completions, /v1/models            │
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
│   crates/domain/mcp, crates/domain/agent,    │
│   crates/domain/marketplace, /evaluation,    │
│   crates/domain/slack, crates/domain/teams   │
│   Authz hook chain, GovernanceEngine, quota, │
│   tool allowlist, MCP policy, route select   │
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
| TB6 | Integration egress | outbound HTTPS to Slack, Microsoft Teams, external MCP servers, authz webhooks, and JWKS endpoints | third-party SaaS and customer-operated tool servers |

## 3. Assets

1. **Bearer credentials** — JWTs, OAuth tokens, refresh tokens. Short-lived; transient in memory during request handling.
2. **Provider API keys and the JWT signing key** — Anthropic, OpenAI, Gemini keys, plus the RSA private key that signs first-party tokens. Loaded from a profile-referenced secrets file or environment into process memory for the lifetime of the process.
3. **MCP allowlist configuration** — signed manifest of permitted tool servers (integrity is load-bearing).
4. **Audit and log events** — record of every governed interaction (integrity and non-repudiation are the product).
5. **User / RBAC data** — who can call what, scoped by handler boundary.
6. **Prompt and response content** — may contain PHI, source code, or other regulated data in-flight (never persisted unless the customer opts in).
7. **Per-user external credentials** — bearer tokens banked for external MCP servers and resolved per-caller through the `external_auth` seam; the deployment's own credential is deliberately withheld from the upstream server.
8. **Governance decision record** — every authorization decision, including denials, written to `governance_decisions` with the actor, context, and delegation chain.

## 4. STRIDE Analysis

Each threat is mapped to the component where it originates, the mitigation in code, and residual risk that remains after mitigation.

### 4.1 Spoofing

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Forged JWT impersonating a user or service | `crates/infra/security/src/jwt/validate.rs` | All first-party token decoding funnels through a single primitive, `decode_rs256_claims` (`jwt/validate.rs:64`). It rejects any header whose `alg` is not `RS256` (`validate.rs:70`) before `Validation::new(Algorithm::RS256)` is even constructed (`:80`) — the JWT plane is RS256-only and there is no ES256/ES384/EdDSA acceptance path anywhere in the codebase, so HS256, `alg: none`, and algorithm-confusion are structurally unrepresentable rather than merely filtered. A `kid` is **mandatory** (`:75`) and is resolved against the in-process `TokenAuthority` (`keys/authority.rs`) plus the JWKS sets fetched for every entry in `profile.security.trusted_issuers`; an unknown `kid` fails closed. Audience is **always** validated: the policy's audience list is applied unconditionally (`:87-88`) and a policy carrying no audiences is rejected outright as `EmptyAudiencePolicy` (`:65-67`), so an "accept any audience" configuration cannot be expressed. Per-surface isolation is enforced via typed `JwtAudience` values (`auth/hook_token.rs:79`) and per-server audience checks in `crates/domain/mcp/src/middleware/rbac.rs:153`. `exp`/`nbf` are checked with a pinned 30s leeway (`JWT_LEEWAY_SECONDS`, `validate.rs:29`), the `act` delegation chain is depth-capped (`auth/validation.rs:39-47`), and `user_type` is re-derived from `scope` with a disagreeing claim rejected (`jwt/decode.rs:42-48`). | Trust in the issuer's private-key custody. Customer IdPs that publish a JWKS can be federated via `trusted_issuers`. The session-context policy pins audience but not issuer (`validate.rs:42-50`), relying on the single-key `kid` lookup instead. Only one signing key is active at a time (`keys/authority.rs:136`), so key rotation has no overlap window and invalidates live tokens — rotate during a maintenance window. |
| OAuth authorisation code interception | `crates/domain/oauth/src/repository/oauth/auth_code/` | PKCE is **mandatory at authorize** — a missing `code_challenge` is an error and a minimum challenge length is enforced (`routes/oauth/endpoints/authorize/validation/mod.rs:130-140`), so the public-client mandate is no longer optional. Only `S256` is accepted; `plain` fails to parse into `PkceMethod`. Verification compares the SHA-256 of the verifier against the stored challenge in constant time via `subtle::ConstantTimeEq` (`auth_code/pkce.rs:41-44`), and every failure mode collapses to the same generic message so the endpoint is not an oracle. Codes are single-use (`used_at`), short-TTL (`expires_at`), and `redirect_uri` must match the original request by **exact string comparison** — no wildcard or prefix matching (`services/validation/redirect_uri.rs:8`). Server-issued `state` is stored HMAC-hashed and consumed atomically in a single `UPDATE`, so replay, expiry and tamper are indistinguishable to the caller (`repository/oauth/state_binding.rs`). Replay of a used code revokes the entire refresh-token family. | At authorize, `redirect_uri` is validated only when supplied; otherwise the client's registered default applies (`authorize/validation/mod.rs:58-68`). OAuth values are peppered at rest (HMAC-SHA256 under `oauth_at_rest_pepper`) but carry no `pepper_version`, so rotating the pepper invalidates every stored row (`at_rest.rs:10-13`). Compromise of the authorising IdP itself cannot be prevented. |
| Provider response substitution | `crates/domain/ai` | TLS via the customer's trust store; response content-type and schema validation | Upstream provider compromise is outside the trust boundary. |
| MCP server impersonation | `crates/domain/mcp`, `crates/infra/security/src/manifest_signing.rs` | Signed manifest allowlist; manifest signature verified at load using the deployment's Ed25519 key (`ed25519_dalek`) over canonical JSON. The `manifest_signing_secret_seed` is distinct from the OAuth signing key. | Manifest-signing key compromise rotates via `systemprompt admin bridge rotate-signing-key`. |

### 4.2 Tampering

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| SQL injection | `crates/infra/database`, all repositories | Request-path queries use `sqlx::query!`, `sqlx::query_as!`, `sqlx::query_scalar!` — compile-time verified against the live schema. This is **mechanically gated, not review-enforced**: `scripts/check-sqlx.sh` (run in CI as `just lint-sqlx`) fails the build on any non-macro `sqlx::query*(` call outside a small allowlist of admin/bootstrap paths. `format!`-constructed DDL exists only in those bootstrap paths, where parameters come from the operator's config file rather than user input. Transaction-scoped Postgres GUCs used by the scoping seam are injection-safe by construction — the key is validated against a custom-GUC grammar and both key and value are bound as parameters (`crates/infra/database/src/scope/provider.rs`). | Verified by the CI gate on every push; it currently passes clean. |
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
| Server-side request forgery via operator- or caller-influenced URLs | `crates/shared/models/src/net.rs`, authz webhook, Slack/Teams clients, external MCP, JWKS fetch | A single canonical guard, `validate_outbound_url` (`net.rs:81`), is applied at every outbound seam: the authz webhook at boot (`authz/runtime.rs:128`), agent webhook delivery, the Slack and Teams clients, the MCP HTTP client, and provider profile URLs. It enforces HTTPS-only (except loopback or an operator-trusted host) and blocks RFC1918, loopback, link-local, unspecified, broadcast, RFC 6598 CGNAT `100.64/10`, and IPv6 unique-local/link-local, unwrapping `::ffff:0:0/96` v4-mapped addresses before applying the v4 blocklist. JWKS fetching adds an explicit host allowlist on top (`keys/jwks_client/fetch.rs:103-109`). | **The guard is parse-time only — it does not resolve DNS** (`net.rs:120` treats any `Host::Domain` as unresolvable and passes it), so a hostname that resolves to a link-local metadata address is not blocked, and it does not re-validate on redirect. `SYSTEMPROMPT_TRUSTED_HTTP_HOSTS` disables the guard per-host and is inherited by subprocesses. The AI-gateway outbound path does not call the guard at all — provider endpoints are trusted because they originate in the operator's profile. Named as a priority target for external assessment. |
| Caller identity leaking to an upstream provider | `crates/entry/api/src/services/gateway/` | The caller's own credential is **never** forwarded upstream — the gateway substitutes the deployment's provider key, resolved by name from the secrets store against the matched route (`gateway/service/resolve.rs:82-92`). `strip_caller_identity` (`stages/outbound.rs:45`) unconditionally removes `metadata.user_id` from the canonical request, and the passthrough lane does the same, so an end-user identifier cannot reach a provider the caller did not choose. Forwarded headers are an explicit allowlist. | Prompt content itself still crosses TB5 by design; that is the governed action, recorded in the audit trail. |
| PHI / regulated content accidentally persisted | `crates/domain/ai` | Prompt and response bodies are not persisted by default; opt-in retention is config-gated. | If a customer enables retention, they inherit responsibility for its lifecycle. |
| Cross-tenant data bleed | `crates/infra/database/src/scope/`, `crates/infra/security/src/authz/`, handler boundary | Isolation today rests on three mechanisms: per-`user_id` filtering in repository queries; the authz resolver, which is deny-overrides and treats a declared ruleset as authoritative and closed at each level of the parent chain, returning `UnknownEntity` rather than a generic deny for unregistered entities (`authz/resolver.rs:94-110`); and subject-keyed quota buckets. Handler-boundary RBAC is enforced for MCP in `crates/domain/mcp/src/middleware/rbac.rs`. | **This is the weakest control in the system and should be read narrowly.** The database scoping layer is a *seam, not an implementation*: `crates/infra/database/src/scope/mod.rs:11-16` states that it is strictly opt-in and that with no registered provider it degenerates to a plain `pool.begin()`. No `ConnectionScopeProvider` is registered anywhere in this repository and **no RLS policies ship in any migration**, so core provides **no row-level tenant isolation**. Deployments requiring hard multi-tenant separation should register a scope provider and author RLS policies, or run one instance per tenant. Cross-tenant access is an explicit target for external assessment rather than a control we claim as covered. |

### 4.5 Denial of Service

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Request flood at entry | `crates/entry/api` | A request body-size limit is wired at the API edge: `DefaultBodyLimit::max(2 MiB)` (`crates/entry/api/src/services/server/builder.rs:44`). IP-keyed rate limiting and bot/ban controls are applied as middleware behind the trusted-proxy client-IP resolver. No global tower `ConcurrencyLimit` or `TimeoutLayer` is wired in the builder; concurrency and timeout bounding is expected at the customer's reverse proxy / load balancer. | High-volume DoS mitigation is operational; the customer typically front-loads with a WAF / LB. |
| Expensive rule evaluation | `crates/app/runtime` | Rule evaluator has bounded complexity; no user-supplied regex/eval. | Complex allowlist configurations increase p99 linearly — benchmark before large rollouts. |
| Upstream provider slowness propagating back pressure | `crates/domain/ai` | Every provider is wrapped in a timeout/retry/circuit-breaker/bulkhead decorator (`crates/domain/ai/src/services/providers/provider_factory.rs`); timeouts are per-provider and configurable. | Governance correctness is preferred over availability — the binary fails closed. |
| Budget exhaustion by a single subject | `crates/entry/api/src/services/gateway/quota.rs` | Spend windows are keyed per subject — by user by default, or by any extension-registered subject dimension such as organisation — and persisted through `AiQuotaBucketRepository`. Extension guards can deny with `GatewayDenyKind::Quota`, returning 429 with an audit row. | **Cost ceilings are enforced one request late** (documented at `quota.rs:5-6`): a single in-flight request can overshoot the budget, because true cost is only known once the provider responds. Size the ceiling with one maximal request of headroom. |
| Audit write pressure | `crates/infra/events` | Batched writes where transactionally safe; async forwarding to SIEM so a slow SIEM cannot block the request path. | Postgres write saturation remains customer-sizeable. |

### 4.6 Elevation of Privilege

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| User escalating scope via token manipulation | `crates/infra/security` | Scopes derive from JWT claims at entry and are immutable through the request lifecycle; `user_type` is re-derived from the permission set and a disagreeing claim is rejected; handler-boundary RBAC re-checks at each crossing. | Relies on IdP claim correctness. |
| Extension gaining undeclared capabilities | `crates/shared/extension` | Extensions register via `inventory` at compile time; the trait surface is typed and narrow; no runtime code load. | The supply chain of compiled-in extensions is the customer's build-time decision. |
| MCP tool invoking outside declared surface | `crates/domain/mcp` | The allowlist gates every tool call; declared capabilities are enforced at call time; the server manifest pins transport and methods. | Tool servers themselves are trusted by the allowlist; customers control what they list. |
| Reaching a route that was never gated | `crates/entry/api/src/services/middleware/` | Attaching the auth context layer is only possible through `RouterExt::with_auth(auth, policy)`, which takes an `AuthzPolicy` in the same call — omitting the policy is a **compile error**, not a review miss, across all 26 call sites. `authz_gate` fails closed: a request arriving with no `RequestContext` is treated as `UserType::Anon`, so only an explicitly `public()` policy admits it. | Some routes are deliberately outside `with_auth` and authenticate in-handler: the AI gateway (bespoke credential extraction), and Slack and Teams (request-signature verification). The MCP proxy uses a deliberately deferred policy so it can emit an RFC 9728 `WWW-Authenticate` challenge. `authz_gate` is a **coarse** gate by design — per-resource ownership checks are the handler's responsibility, so a handler that forgets one is not caught by the compile-time guarantee. |
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
- **No third-party penetration test has been commissioned to date.** Because the binary runs entirely inside the customer's own compliance boundary and we never receive customer data, the assessment that carries the most weight is one the customer runs against their own deployment — the source is available for review and customer-commissioned testing is welcomed with reasonable coordination. A commissioned external assessment is tracked in [rfi-readiness-audit.md §6](rfi-readiness-audit.md); the surfaces it should prioritise are cross-tenant isolation, the MCP and agent protocol surface, and the gateway's outbound path.

## 7. Change Log

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
| 2026-08-28 | Fidelity pass against `next` @ 0.41.0 after ~29 minors of drift. **Closed two residual risks that had been fixed in code but were still advertised as open:** audience validation is now mandatory and per-surface (an empty audience policy is rejected outright), and PKCE is mandated server-side at authorize with `plain` rejected. Modelled the multi-dialect gateway surface (`/v1/responses`, `/v1/chat/completions`, `/v1/models`) and the four post-0.12 domains, none of which had appeared in any trust boundary, asset list, or STRIDE row; added TB6 for integration egress. Added STRIDE rows for SSRF, caller-identity leakage to providers, quota exhaustion, and ungated-route reachability. Recorded the SSRF guard's parse-time-only DNS limitation as residual risk. **Rewrote the cross-tenant row to state plainly that database scoping is an unimplemented seam and core ships no row-level tenant isolation** — the prior wording implied more than the code delivers. Repointed dead citations (`auth/validation.rs:92` → `jwt/validate.rs:70`; `auth_code.rs:238-275` → `auth_code/pkce.rs:41-44`; `builder.rs:93` → `:44`). Recorded that no third-party penetration test has been commissioned. |
| 2026-05-22 | Corrected DoS row to the body-size limit actually wired (`DefaultBodyLimit::max(2 MiB)`); removed unverified concurrency/timeout-layer claim. Confirmed RS256-only JWT plane and the RBAC path. Repointed secrets-bootstrap citations to `crates/infra/config/src/bootstrap/secrets/`. Marked audit-table grant as operator-provisioned, and `validate_aud=false` / PKCE-mandate as tracked open items. |
