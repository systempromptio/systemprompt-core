# Compatibility Matrix

Upstream API, protocol, and runtime versions supported by each systemprompt.io release.

## Current — 0.11.x

### AI Provider APIs

| Provider | API / version | Supported models | Notes |
|----------|---------------|------------------|-------|
| Anthropic | Messages API (`/v1/messages`) | Claude 4.7 Opus, Claude 4.6 Sonnet, Claude 4.5 Haiku, Claude 3.5 / 3.7 family | Streaming, tool use, thinking, prompt caching all supported |
| OpenAI | Chat Completions, Responses API | GPT-4o, GPT-4.1, o1, o3 family | Streaming, tool calling, JSON mode |
| Google | Gemini `generateContent` | Gemini 2.x Pro / Flash | Streaming, tool calling |
| Self-hosted | OpenAI-compatible endpoints | vLLM, TGI, Ollama, llama.cpp | Configurable `base_url` |

Provider adapters are tracking surface per [stability-contract.md §2.1](../security/stability-contract.md). New provider features appear in point releases.

### Protocols

| Protocol | Tracked revision | Notes |
|----------|------------------|-------|
| MCP (Model Context Protocol) | 1.6 (via `rmcp` 1.6) | Streamable-HTTP and stdio transports; signed manifest allowlist for server identity |
| A2A (Agent-to-Agent) | 0.2.x | Task / Message / TaskState types per current public spec |
| OAuth 2.x / OIDC | PKCE S256 (RFC 7636); OIDC 1.0 Core | PKCE required for the authorisation code flow; discovery + standard claims |
| Prometheus exposition | 0.0.4 text format | via `/metrics` endpoint (always mounted) |

### Runtime

| Component | Version |
|-----------|---------|
| Rust toolchain | pinned via `rust-toolchain.toml` (nightly) |
| Rust edition | 2024 |
| PostgreSQL | 18+ |
| Minimum glibc (Linux binaries) | 2.28 |
| Tokio | 1.49 |
| Axum | 0.8 |
| SQLx | 0.8 (postgres, compile-time macros, rustls) |
| rmcp | 1.6 |
| webauthn-rs | 0.5 |

### Release Targets

Pre-built binaries published per release:

- `aarch64-apple-darwin` (macOS, Apple Silicon)
- `x86_64-apple-darwin` (macOS, Intel)
- `x86_64-pc-windows-msvc` (Windows)
- `x86_64-unknown-linux-gnu` (Linux)

Other targets are buildable from source.

## Historical

| systemprompt version | Status |
|----------------------|--------|
| 0.11.x | Current supported line. |
| 0.10.x | Prior line; security fixes only. |
| < 0.10 | No longer supported. |

Per-release detail is in `CHANGELOG.md`.

## Compatibility Commitments

- **Within a minor series**, the supported matrix above only grows. New provider models, new protocol revisions, and new release targets are additive.
- **Removing a supported provider model** requires a `BREAKING` CHANGELOG entry and a one-minor deprecation window.
- **Removing support for a Postgres major version** requires the same.
- **Upstream provider deprecations** propagate at the provider's cadence, not ours. When a provider retires a model, the deprecation is surfaced in the CHANGELOG and in runtime warnings; requests are not blocked until the provider does.

## Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
| 2026-05-22 | Corrected axum 0.7.x → 0.8 and sqlx 0.7.x → 0.8 against root `Cargo.toml`. Pinned Tokio to 1.49, Postgres to 18+, and added rmcp 1.6 / webauthn-rs 0.5. Restated the MCP revision as 1.6 (the version tracked via `rmcp`) and the OAuth row as OAuth 2.x / OIDC with PKCE S256. |
