# Compatibility Matrix

Upstream API, protocol, and runtime versions supported by each systemprompt.io release.

## Current — 0.43.x

### AI Provider APIs

systemprompt does not ship a model catalogue. Which providers and models a deployment
exposes is determined entirely by `profile.providers` and `gateway.routes` in the operator's
own configuration, so the table below describes the **API surfaces** the adapters speak, not
an allowlist of model names. New models from a provider work as soon as the operator adds
them to a route; no release of systemprompt is required.

| Provider | API surface | Notes |
|----------|-------------|-------|
| Anthropic | Messages API (`/v1/messages`) | Streaming, tool use, extended thinking, prompt caching |
| OpenAI | Chat Completions and Responses API | Streaming, tool calling, JSON mode. Uses `max_completion_tokens` |
| Google | Gemini `generateContent` | Streaming, tool calling |
| Self-hosted | Any OpenAI-compatible endpoint | vLLM, TGI, Ollama, llama.cpp — configurable `base_url` |

Provider adapters are tracking surface per [stability-contract.md §2.1](../security/stability-contract.md).
New provider features appear in point releases.

### Inbound protocol compatibility

Distinct from the outbound adapters above: the gateway also **accepts** requests in more
than one provider dialect, so existing client SDKs can be pointed at systemprompt without
code changes.

| Inbound endpoint | Dialect | Since |
|------------------|---------|-------|
| `/v1/messages` | Anthropic Messages | 0.20.x |
| `/v1/responses` | OpenAI Responses | 0.26.x |
| `/v1/chat/completions` | OpenAI Chat Completions | 0.40.0 |
| `/v1/models` | Model listing, filtered by `x-inference-protocol` | 0.26.x |

### Protocols

| Protocol | Tracked revision | Notes |
|----------|------------------|-------|
| MCP (Model Context Protocol) | `2026-07-28` | Negotiated explicitly, not via `ProtocolVersion::LATEST`. Older revisions down to `2025-06-18` negotiate down. Streamable-HTTP and stdio transports; signed manifest allowlist for server identity. Carried by `rmcp` 3.1.3 |
| A2A (Agent-to-Agent) | 0.3.0 | `A2A_PROTOCOL_VERSION`; Task / Message / TaskState types per the current public spec |
| OAuth 2.x / OIDC | PKCE S256 (RFC 7636); OIDC 1.0 Core | PKCE mandatory for the authorisation code flow; `plain` is rejected. Discovery + standard claims. RFC 7009 revocation, RFC 9728 protected-resource metadata |
| Prometheus exposition | 0.0.4 text format | via `/metrics` (always mounted) |

MCP revisions are **dates, not semantic versions** — `2026-07-28` is a protocol revision
identifier, not a release number.

### Runtime

| Component | Version |
|-----------|---------|
| Rust toolchain | `nightly-2026-06-03`, pinned in `rust-toolchain.toml` |
| Rust edition | 2024 |
| Minimum supported Rust version (MSRV) | 1.96 — declared as `rust-version` and enforced by a dedicated CI job |
| PostgreSQL | 18+ (see note below) |
| Minimum glibc (Linux binaries) | 2.35 — release binaries build on `ubuntu-22.04` to pin the oldest supported runner glibc |
| Tokio | 1.53 |
| Axum | 0.8 |
| SQLx | 0.9 (postgres, compile-time macros, rustls) |
| rmcp | 3.1.3 (exact pin) |
| jsonwebtoken | 10 (`rust_crypto` backend) |
| reqwest | 0.12 (rustls) |
| webauthn-rs | 0.5 |

> **Note on the PostgreSQL floor.** 18+ is the documented and supported requirement, and is
> what the production deployment guide assumes. The CI test matrix currently provisions
> PostgreSQL 16 containers, so the 18+ floor is not exercised by automated tests. Reconciling
> the two — either raising the CI containers or lowering the documented floor to what is
> actually tested — is tracked in
> [rfi-readiness-audit.md §6](../security/rfi-readiness-audit.md).

### Release Targets

Pre-built, signed `systemprompt-bridge` binaries published per `bridge-v*` release:

- `aarch64-apple-darwin` (macOS, Apple Silicon)
- `x86_64-pc-windows-msvc` (Windows)
- `x86_64-unknown-linux-gnu` (Linux, glibc 2.35+)

The core platform is distributed as source and as `systemprompt-*` crates on crates.io
rather than as pre-built binaries. Other targets are buildable from source.

## Historical

| systemprompt version | Status |
|----------------------|--------|
| 0.43.x | Current supported line. |
| 0.42.x | Prior line; Critical and High fixes only. |
| < 0.42 | No longer supported. |

Per-release detail is in `CHANGELOG.md`.

## Compatibility Commitments

- **Within a minor series**, the supported matrix above only grows. New provider models, new protocol revisions, and new release targets are additive.
- **Removing a supported provider API surface** requires a `BREAKING` CHANGELOG entry and a one-minor deprecation window.
- **Removing support for a Postgres major version** requires the same.
- **Upstream provider deprecations** propagate at the provider's cadence, not ours. When a provider retires a model, the deprecation is surfaced in the CHANGELOG and in runtime warnings; requests are not blocked until the provider does.

## Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
| 2026-05-22 | Corrected axum 0.7.x → 0.8 and sqlx 0.7.x → 0.8 against root `Cargo.toml`. Pinned Tokio to 1.49, Postgres to 18+, and added rmcp 1.6 / webauthn-rs 0.5. Restated the MCP revision as 1.6 (the version tracked via `rmcp`) and the OAuth row as OAuth 2.x / OIDC with PKCE S256. |
| 2026-08-28 | Full fidelity pass against `next` @ 0.41.0 after ~29 minors of drift. Corrected Tokio 1.49 → 1.53, SQLx 0.8 → 0.9, glibc 2.28 → 2.35, and the current line 0.39.x → 0.41.x. Replaced the incorrect "MCP 1.6 (via rmcp 1.6)" with the real tracked revision `2026-07-28` carried by `rmcp` 3.1.3, and corrected A2A 0.2.x → 0.3.0. Added the MSRV (1.96), the toolchain pin date, and `jsonwebtoken` — the crate the one accepted RSA advisory depends on. Added the inbound protocol-compatibility table, which had no entry despite the OpenAI-compatible surface shipping in 0.40.0. Replaced the hardcoded model lists with the configuration-driven reality. Corrected the release-target list to the three targets actually built, and clarified that they are bridge binaries. Flagged the documented-vs-tested PostgreSQL floor. |
