# Stability Contract

This document defines what is stable in systemprompt.io and what is not. It describes compatibility expectations for consumers.

## Current Version

`0.49.x` across the workspace. See root `Cargo.toml` for the exact current version.

The workspace uses pre-1.0 versioning. The sections below distinguish maintained interfaces from provider and protocol adapters that track upstream changes. Version-specific breaking changes and migration requirements are recorded in the changelog.

## 1. Stable Surface

The following surfaces are considered stable today. Breaking changes to these require a major version bump, a documented migration path, and at least 12 months of deprecation notice once `1.0` ships. Pre-`1.0` they are treated with the same discipline as a post-`1.0` stable surface, with changes called out in `CHANGELOG.md` under a `BREAKING` tag.

### 1.1 Governance API (HTTP surface)

- `POST /v1/messages` — Anthropic-dialect inference. Request and response shapes, error codes, HTTP semantics
- `GET /livez` and `GET /readyz` — liveness and readiness probes (unauthenticated)
- `GET /health` and `GET /api/v1/health` — health summaries (unauthenticated)
- `GET /api/v1/health/detail` — rich health detail (authenticated)
- `GET /metrics` — Prometheus scrape format (metric names and labels); served on `server.metrics_port` when configured; restrict access to that listener
- OAuth2/OIDC discovery and callback routes under `/api/v1/core/oauth` and `/.well-known/`

Use `/livez` for unauthenticated process liveness and `/readyz` for bootstrap, database and shutdown readiness. `/health` provides an operational summary; `/api/v1/health/detail` requires authentication.

### 1.2 Audit Event Schema

The structure of audit events written by `crates/infra/events` is stable:

- Event type names
- Field names, types, and semantics
- Append-only table schema

Additions are allowed without notice; removal or rename is a breaking change.

### 1.3 Configuration Schema

The `Config` struct (`crates/shared/models/src/config/mod.rs`) and the YAML profile schema:

- Top-level keys and their semantics
- Required vs. optional fields
- Profile bootstrap sequence (`ProfileBootstrap` → `SecretsBootstrap` → `CredentialsBootstrap` → `Config` → `AppContext`)

New optional fields are additive. Required fields cannot be added without a major bump.

### 1.4 Database Schema

DDL for tables that persist customer-observable state:

- User / tenant / identity tables
- Audit tables
- OAuth state tables
- MCP server registry

Migrations are **additive-only within a minor series**. A rolling upgrade from `0.45.N` to `0.45.N+1` is always safe. See deployment guide §9 for rollback semantics.

### 1.5 Extension Framework

Public traits in `crates/shared/extension/`:

- the `Extension` trait (`traits/extension.rs`)
- `ExtensionMetadata`, `SchemaDefinition`, `ExtensionRouter`, `Migration` shapes
- the `register_extension!` and `extension_migrations!` macro contracts
- the capability traits an extension declares against (`HasConfig`, `HasDatabase`, `HasHttpClient`, `HasEventBus`, and peers in `capabilities.rs`) and the `ExtensionContext` trait


### 1.6 Typed Identifiers

`crates/shared/identifiers/` — `UserId`, `TaskId`, `TenantId`, etc. Wire formats are type-specific; preserve each identifier's serialization contract when integrating.

### 1.7 CLI

`systemprompt` CLI subcommand names and their primary flags (see `crates/entry/cli/`). Secondary flags may be deprecated with one minor's notice.

## 2. Tracking Surface

The following deliberately move as the upstream ecosystem moves. They are versioned by the [compatibility matrix](../reference/compatibility.md) and their changes are covered by point releases, not major bumps.

### 2.1 Provider Adapters

`crates/domain/ai` provider submodules — Anthropic, OpenAI, Gemini, and any future provider adapter. These track:

- Upstream API schemas (request/response shapes)
- New features as providers ship them (prompt caching, thinking, tool use variants, batch, files, citations)
- Model name lists and capability flags
- Rate-limit and retry semantics per provider

A customer using the governance API does not call provider adapters directly — the governance API is the stable abstraction over them. Adapter changes land in point releases and are noted in `CHANGELOG.md`.

### 2.2 MCP Protocol Support

`crates/domain/mcp/` tracks the evolving Model Context Protocol specification. The MCP allowlist and manifest-signing format is stable, but the protocol-level primitives (resource templates, prompt templates, sampling, new method namespaces) move as MCP moves.

### 2.3 A2A Protocol

`crates/domain/agent/` implements the A2A (agent-to-agent) protocol. Message / Task / TaskState types follow the protocol spec revisions.

### 2.4 Inbound Provider Dialects

The gateway accepts requests in more than one provider dialect so that existing client SDKs
can be pointed at systemprompt unchanged:

- `POST /v1/messages` (Anthropic) is **Stable Surface** — see §1.1
- `POST /v1/responses` and `POST /v1/chat/completions` (OpenAI) are **tracking surface**. They
  mirror upstream OpenAI request/response shapes, so they move when those shapes move.
- `GET /v1/models` is tracking surface; the listing is filtered per `x-inference-protocol`

### 2.5 Newer Domains

The following ship as functional surface but are **not yet classified as stable**, and may
change shape within a minor while they settle:

- `crates/domain/marketplace` — plugin and skill catalogue, ABAC attribute floor
- `crates/domain/evaluation` — sample / judge / replay framework
- `crates/domain/slack` and `crates/domain/teams` — outbound messaging integrations

### 2.6 Internal Implementation

Anything inside `crates/` that is not exported through the public surfaces above is implementation detail. Refactors, rewrites, and module reorganisations are allowed without notice.

## 3. Customer Commitments

For a customer on a supported version:

1. **Within a minor series (e.g. `0.45.0` → `0.45.7`):** no breaking changes to the Stable Surface. Rolling upgrades are safe. Database migrations are additive-only. Rollback to the immediately prior minor is supported.
2. **Across minors (e.g. `0.44.x` → `0.48.x`):** breaking changes are possible only on the Stable Surface with a `BREAKING` entry in `CHANGELOG.md`, migration notes, and a deprecation window of at least one prior minor where both forms were accepted. Database migrations between minors are forward-compatible by design; rollback to the prior minor is supported. <!-- version-ok: worked example -->
3. **Upstream provider API changes:** handled in point releases; the governance API shields customers from most of these. When a provider ships a change that cannot be absorbed transparently, it becomes a new optional field in the governance API.
4. **Security fixes:** delivered per the SECURITY.md SLAs regardless of minor boundary.
5. **Licence stability:** BUSL-1.1 with four-year conversion to Apache 2.0. The conversion commitment is permanent.

## 4. Path to 1.0

`1.0` will be cut when:

- The Governance API (§1.1) has been unchanged for at least one minor cycle
- The MCP protocol revision tracked has reached a stable published version
- The A2A protocol tracked has reached a stable published version
- Customer-facing upgrade friction has been demonstrably low across at least one minor transition

A 1.0 release requires review of these criteria against the supported protocol revisions and upgrade tests.

## 5. Reporting Stability Issues

If you find a stable-surface change that shipped without a `BREAKING` notice, report it via the SECURITY.md channel or open a GitHub issue. Undocumented stability breaks are treated as defects and fixed in the next point release.
