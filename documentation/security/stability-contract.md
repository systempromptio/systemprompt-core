# Stability Contract

This document defines what is stable in systemprompt.io and what is not. It is the honest answer to "you're on 0.3.x — is this safe to build against?"

## Current Version

`0.3.x` across the workspace. See root `Cargo.toml` for the exact current version.

We have not cut `1.0`. The reason is specific and deliberate: systemprompt integrates with AI provider APIs (Anthropic Messages API, OpenAI Chat Completions, Gemini, MCP spec, A2A protocol) that are themselves evolving rapidly, often under research-preview terms. Declaring `1.0` while our upstream surface is still in motion would claim a level of stability we cannot honestly provide for the full binary. We would rather be plainly `0.3` and truthful about the shape of stability than pretend.

What the rest of this document does is separate the parts of systemprompt that **are** stable from the parts that track upstream and must be allowed to move. The customer-facing commitment is in §3 below.

## 1. Stable Surface

The following surfaces are considered stable today. Breaking changes to these require a major version bump, a documented migration path, and at least 12 months of deprecation notice once `1.0` ships. Pre-`1.0` they are treated with the same discipline as a post-`1.0` stable surface, with changes called out in `CHANGELOG.md` under a `BREAKING` tag.

### 1.1 Governance API (HTTP surface)

- `POST /v1/messages` — request and response shapes, error codes, HTTP semantics
- `GET /health/live`, `GET /health/ready` — health probes
- `GET /metrics` — Prometheus scrape format (metric names and labels)
- OAuth2/OIDC discovery and callback routes

### 1.2 Audit Event Schema

The structure of audit events written by `crates/infra/events` is stable:

- Event type names
- Field names, types, and semantics
- Append-only table schema

Additions are allowed without notice; removal or rename is a breaking change.

### 1.3 Configuration Schema

The `Config` struct (`crates/shared/models/src/config.rs`) and YAML profile schema:

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

Migrations are **additive-only within a minor series**. A rolling upgrade from `0.3.N` to `0.3.N+1` is always safe. See deployment guide §9 for rollback semantics.

### 1.5 Extension Framework

Public traits in `crates/shared/extension/`:

- `Extension` and its subtraits (`SchemaExtensionTyped`, `ApiExtensionTyped`, `JobExtensionTyped`, `ProviderExtensionTyped`)
- `ExtensionMetadata`, `SchemaDefinition`, `ExtensionRouter` shapes
- `register_extension!` macro contract

### 1.6 Typed Identifiers

`crates/shared/identifiers/` — `UserId`, `TaskId`, `TenantId`, etc. Their wire format (string prefix + ULID/UUID) is stable.

### 1.7 CLI

`systemprompt` CLI subcommand names and their primary flags (see `crates/entry/cli/`). Secondary flags may be deprecated with one minor's notice.

## 2. Tracking Surface

The following deliberately move as the upstream ecosystem moves. They are versioned by the [compatibility-matrix.md](compatibility-matrix.md) and their changes are covered by point releases, not major bumps.

### 2.1 Provider Adapters

`crates/domain/ai` provider submodules — Anthropic, OpenAI, Gemini, and any future provider adapter. These track:

- Upstream API schemas (request/response shapes)
- New features as providers ship them (prompt caching, thinking, tool use variants, batch, files, citations)
- Model name lists and capability flags
- Rate-limit and retry semantics per provider

A customer using the governance API does not call provider adapters directly — the governance API is the stable abstraction over them. Adapter changes land in point releases and are noted in `CHANGELOG.md`.

### 2.2 MCP Protocol Support

`crates/domain/mcp/` tracks the evolving Model Context Protocol specification. The MCP allowlist and manifest signing format is stable, but the protocol-level primitives (resource templates, prompt templates, sampling, new method namespaces) move as MCP moves.

### 2.3 A2A Protocol

`crates/domain/agent/` implements the A2A (Agent-to-Agent) protocol. Message / Task / TaskState types follow the protocol spec revisions.

### 2.4 Internal Implementation

Anything inside `crates/` that is not exported through the public surfaces above is implementation detail. Refactors, rewrites, and module reorganisations are allowed without notice.

## 3. Customer Commitments

For a customer on a supported version:

1. **Within a minor series (e.g. `0.3.0` → `0.3.7`):** no breaking changes to the Stable Surface. Rolling upgrades are safe. Database migrations are additive-only. Rollback to the immediately prior minor is supported.
2. **Across minors (e.g. `0.3.x` → `0.4.x`):** breaking changes possible only on the Stable Surface with a `BREAKING` entry in `CHANGELOG.md`, migration notes, and a deprecation window of at least one prior minor where both forms were accepted. Database migrations between minors are forward-compatible by design; rollback to the prior minor is supported.
3. **Upstream provider API changes:** handled in point releases; the governance API shields customers from most of these. When a provider ships a change that cannot be absorbed transparently (e.g. new required parameters, new capability flags customers want to opt into), it becomes a new optional field in the governance API.
4. **Security fixes:** delivered per SECURITY.md SLAs regardless of minor boundary.
5. **Licence stability:** BSL-1.1 with four-year conversion to Apache 2.0. The conversion commitment is permanent.

## 4. Path to 1.0

`1.0` will be cut when:

- The Governance API (§1.1) has been unchanged for at least one minor cycle
- The MCP protocol revision we track has reached a stable published version
- The A2A protocol we track has reached a stable published version
- Customer-facing upgrade friction has been demonstrably low across at least one minor transition

At `1.0` the commitments in §3 become semver-formal. The intent is to reach `1.0` in the second half of 2026, but the date is outcome-driven, not calendar-driven.

## 5. Reporting Stability Issues

If you find a stable-surface change that shipped without a `BREAKING` notice, report it via the SECURITY.md channel or open a GitHub issue. We treat undocumented stability breaks as defects and fix them in the next point release.

## 6. Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
