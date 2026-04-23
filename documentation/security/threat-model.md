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
│   crates/infra/events (structured audit),    │
│   crates/infra/security (ChaCha20-Poly1305), │
│   crates/infra/database (sqlx + Postgres)    │
└──────────────────────────────────────────────┘
    │
    ▼
PostgreSQL (customer-managed)
```

Architectural detail: [../../instructions/information/architecture.md is internal; see crate layout in the repository root].

## 2. Trust Boundaries

| # | Boundary | Inside | Outside |
|---|----------|--------|---------|
| TB1 | Network edge | customer VPC / cluster | developers, agents, CI jobs |
| TB2 | Process boundary | systemprompt binary | OS, Postgres, provider SDKs |
| TB3 | Secrets boundary | encrypted at rest via ChaCha20-Poly1305 | plaintext in memory during use |
| TB4 | Audit boundary | append-only structured event stream | downstream SIEM / object store |
| TB5 | Provider egress | customer-controlled outbound HTTPS to providers | provider cloud |

## 3. Assets

1. **Bearer credentials** — JWTs, OAuth tokens, refresh tokens (short-lived; never persisted in plaintext).
2. **Provider API keys** — Anthropic, OpenAI, Gemini keys (at-rest encryption; decrypted only at egress time).
3. **MCP allowlist configuration** — signed manifest of permitted tool servers (integrity is load-bearing).
4. **Audit events** — immutable record of every governed interaction (integrity and non-repudiation are the product).
5. **User / RBAC data** — who can call what, scoped by handler boundary.
6. **Prompt and response content** — may contain PHI, source code, or other regulated data in-flight (never persisted unless customer opts in).

## 4. STRIDE Analysis

Each threat is mapped to the component where it originates, the mitigation in code, and residual risk that remains after mitigation.

### 4.1 Spoofing

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Forged JWT impersonating a user or service | `crates/infra/security` | Verify signature against configured JWKS; enforce `iss`, `aud`, `exp`, `nbf`; reject `alg: none` explicitly | Depends on customer IdP key hygiene. Rotation policy documented in deployment guide. |
| OAuth authorisation code interception | `crates/domain/oauth` | PKCE required (`S256`), single-use codes, short TTL, `state` parameter validation | Cannot prevent compromise of the authorising IdP itself. |
| Provider response substitution | `crates/domain/ai` | TLS pinning-eligible via customer trust store; response content-type and schema validation | Upstream provider compromise is outside our trust boundary. |
| MCP server impersonation | `crates/domain/mcp` | Signed manifest allowlist — server identity bound to manifest-declared pubkey/hash | Manifest-signing key compromise rotates via config reload. |

### 4.2 Tampering

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| SQL injection | `crates/infra/database`, all repositories | 100% of queries use `sqlx::query!`, `sqlx::query_as!`, `sqlx::query_scalar!` — compile-time verified against schema; no runtime string interpolation into SQL | Verified by `rg "sqlx::query\b"` returning zero matches outside generated macros. |
| Audit event tampering in-flight | `crates/infra/events` | Events written synchronously within the request transaction; append-only schema (no UPDATE/DELETE on audit tables); downstream SIEM forward is idempotent and sequence-numbered | Post-write DB compromise not addressed here — see 4.4 below. |
| MCP allowlist tampering | `crates/domain/mcp` + config | Manifest loaded from signed source; signature verified at load; hot reload re-verifies | Requires key management discipline on customer side. |
| Prompt/response modification in transit | entry | TLS 1.2+ required (customer-configured certificate); no plaintext HTTP listener | TLS downgrade protected by customer reverse proxy config. |

### 4.3 Repudiation

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| User denies having issued a governed request | `crates/infra/events` | Every request is bound at entry to an authenticated identity; audit row carries JWT `sub`, request ID, timestamp, full rule-evaluation trace | Relies on customer's IdP logs for the identity-to-human binding. |
| Tool invocation cannot be attributed | `crates/domain/mcp`, `crates/domain/agent` | A2A `Task.contextId` + `Message.messageId` propagate through the call graph; audit events reference these IDs | Customer must preserve audit retention for forensic window. |

### 4.4 Information Disclosure

| Threat | Component | Mitigation | Residual |
|--------|-----------|------------|----------|
| Secrets leaked via logs or error traces | `crates/infra/logging` | Structured tracing with field-level redaction; provider keys and tokens tagged as `secret` and omitted from default log output | Third-party crate logs are allowlisted at `INFO`; custom code review required when adding new dependencies. |
| Provider API keys exposed at rest | `crates/infra/security` | ChaCha20-Poly1305 authenticated encryption with per-deployment master key; keys decrypted only within the egress request scope | Master key protection is the customer's responsibility (HSM / KMS / sealed file — see deployment guide). |
| Audit data exfiltration via DB compromise | `crates/infra/database` | DB user for systemprompt has least-privilege (`SELECT, INSERT` on audit; no `DELETE`); recommend separate read-only DB user for SIEM export | Database compromise remains a high-impact event — mitigated operationally, not architecturally. |
| PHI / regulated content accidentally persisted | `crates/domain/ai` | Prompt and response bodies are not persisted by default; opt-in retention is config-gated and rate-limited; retention policy configurable per tenant | If a customer enables retention, they inherit responsibility for its lifecycle. |
| Cross-tenant data bleed | `crates/domain/users`, handler boundary | Handler-boundary RBAC; every repository query scopes by `tenant_id`; integration tests enforce scoping | Depends on correct handler wiring — covered by the test suite. |

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
