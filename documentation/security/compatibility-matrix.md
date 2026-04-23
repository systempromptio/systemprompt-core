# Compatibility Matrix

Upstream API, protocol, and runtime versions supported by each systemprompt.io release.

## Current — 0.3.x

### AI Provider APIs

| Provider | API / version | Supported models | Notes |
|----------|---------------|------------------|-------|
| Anthropic | Messages API (`/v1/messages`) | Claude 4.7 Opus, Claude 4.6 Sonnet, Claude 4.5 Haiku, Claude 3.5 / 3.7 family | Streaming, tool use, thinking, prompt caching all supported |
| OpenAI | Chat Completions, Responses API | GPT-4o, GPT-4.1, o1, o3 family | Streaming, tool calling, JSON mode |
| Google | Gemini `generateContent` | Gemini 2.x Pro / Flash | Streaming, tool calling |
| Self-hosted | OpenAI-compatible endpoints | vLLM, TGI, Ollama, llama.cpp | Configurable `base_url` |

Provider adapters are tracking surface per [stability-contract.md §2.1](stability-contract.md). New provider features appear in point releases.

### Protocols

| Protocol | Tracked revision | Notes |
|----------|------------------|-------|
| MCP (Model Context Protocol) | 2025-06 / 2025-11 | Both stdio and SSE transports; signed manifest allowlist for server identity |
| A2A (Agent-to-Agent) | 0.2.x | Task / Message / TaskState types per current public spec |
| OAuth 2.1 | RFC 9207 + PKCE (RFC 7636) | Required for authorisation code flow |
| OIDC | 1.0 Core | Discovery + standard claims |
| Prometheus exposition | 0.0.4 text format | via `/metrics` endpoint |

### Runtime

| Component | Version |
|-----------|---------|
| Rust toolchain | pinned via `rust-toolchain.toml` |
| Rust edition | 2024 |
| PostgreSQL | 15, 16, 17 |
| Minimum glibc (Linux binaries) | 2.28 |
| Tokio | 1.x |
| Axum | 0.7.x |
| SQLx | 0.7.x |

### Release Targets

Pre-built signed binaries published per release:

- `aarch64-apple-darwin` (macOS, Apple Silicon)
- `x86_64-apple-darwin` (macOS, Intel)
- `x86_64-pc-windows-msvc` (Windows)
- `x86_64-unknown-linux-gnu` (Linux)

Other targets buildable from source.

## Historical

| systemprompt version | Released | Notes |
|----------------------|----------|-------|
| 0.3.0 | 2026-04 | Current. Gateway tracing fixes, extension-framework refinements. See CHANGELOG.md. |
| 0.2.x | 2026-03 → 2026-04 | Prior supported line. Security fixes through 2026-07. |
| < 0.2 | — | No longer supported. |

## Compatibility Commitments

- **Within a minor series**, the supported matrix above only grows. New provider models, new protocol revisions, and new release targets are additive.
- **Removing a supported provider model** requires a `BREAKING` CHANGELOG entry and a one-minor deprecation window.
- **Removing support for a Postgres major version** requires the same.
- **Upstream provider deprecations** propagate at the provider's cadence, not ours. When a provider retires a model, we surface the deprecation in the CHANGELOG and in runtime warnings; we do not block requests until the provider does.

## Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
