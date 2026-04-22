<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo-dark.svg" alt="systemprompt.io" width="400">
</picture>

# Run Claude for Work on your own infrastructure, with your own choice of inference.

`systemprompt-core` is the Rust library that compiles into a single ~50 MB binary. Install it, point your Claude-for-Work fleet's `api_external_url` at it, and every Claude Desktop request lands on a host **you operate** — on your network, in your air-gap, under your audit table. Pick the upstream per model pattern: Anthropic, OpenAI, Gemini, Moonshot (Kimi), Qwen, MiniMax, or a custom provider you register yourself via the `inventory` crate. One YAML block swaps it.

Every tool call authenticated, scoped, secret-scanned, rate-limited, and audited. Compile-time plugin model, compile-time verified SQL, zero-raw-String IDs. BSL-1.1 source-available; Apache 2.0 after four years.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-template/main/demo/recording/svg/output/dark/int-benchmark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-template/main/demo/recording/svg/output/light/int-benchmark.svg">
  <img alt="Governance benchmark: 3,308 req/s" src="https://raw.githubusercontent.com/systempromptio/systemprompt-template/main/demo/recording/svg/output/dark/int-benchmark.svg" width="100%">
</picture>

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg?style=flat-square)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt?style=flat-square)](https://docs.rs/systemprompt)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75+-f97316?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL 18+](https://img.shields.io/badge/postgres-18+-336791?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Template](https://img.shields.io/badge/evaluate-systemprompt--template-16a34a?style=flat-square)](https://github.com/systempromptio/systemprompt-template)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2.svg?style=flat-square)](https://discord.gg/wkAbSuPWpr)

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Live Demo**](https://systemprompt.io/features/demo) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

Building with this? [⭐ Star the repo](https://github.com/systempromptio/systemprompt-core) — helps other Rust developers find it.

</div>

---

- **Embed it** — `systemprompt = { version = "0.3.0", features = ["full"] }` in `Cargo.toml`, then jump to [Extensions (technical)](#extensions-technical) for the compile-time plugin model.
- **Evaluate it running** — clone [`systemprompt-template`](https://github.com/systempromptio/systemprompt-template) for a turnkey demo. `just build && just setup-local <key> && just start` runs 40+ scripted demos against the live binary.

---

## What's new in v0.3.0

**LLM Gateway — `/v1/messages` inference routing.** Organisations using Claude for Work (formerly Claude Cowork) can set `api_external_url` in their fleet MDM configuration to a systemprompt-backed host and have every Claude Desktop inference request flow through the gateway. The gateway:

- Exposes `POST /v1/messages` at the Anthropic wire format — fully compatible with the Claude API SDK, Claude Desktop, and any Anthropic-SDK client.
- Authenticates with a systemprompt JWT in the `x-api-key` header (falls back to `Authorization: Bearer`). No new credential type — existing user JWTs serve as the gateway credential.
- Routes by `model_pattern` to any configured upstream. Built-in provider tags: `anthropic`, `openai` (OpenAI-compatible), `moonshot` (Kimi), `qwen`, `gemini` (stub), `minimax`.
- **Anthropic upstream**: transparent byte proxy. Raw request bytes forwarded verbatim to the upstream endpoint with the upstream API key substituted; the response stream is piped back unmodified. Preserves extended thinking blocks, cache-control headers, and all Anthropic-specific SSE events exactly.
- **OpenAI-compatible upstream**: converts Anthropic request format → OpenAI `/v1/chat/completions`, proxies to the upstream, converts the response back to Anthropic format. Streaming maps OpenAI SSE delta events to Anthropic `message_start` / `content_block_start` / `content_block_delta` / `message_delta` / `message_stop` SSE frames.
- **API key resolution**: upstream API keys resolve from the existing secrets file by secret name (`api_key_secret` in the route config). No new credential storage mechanism.
- **Conditional mount**: the `/v1` router mounts only when `gateway.enabled: true` in the active profile — zero overhead for deployments that don't use the gateway.

**Gateway profile configuration schema.** New `gateway` block in profile YAML (all fields optional; block absent = gateway disabled):

```yaml
gateway:
  enabled: true
  routes:
    - model_pattern: "claude-*"
      provider: anthropic
      endpoint: "https://api.anthropic.com/v1"
      api_key_secret: "anthropic_api_key"
    - model_pattern: "moonshot-*"
      provider: moonshot
      endpoint: "https://api.moonshot.cn/v1"
      api_key_secret: "kimi_api_key"
      upstream_model: "moonshot-v1-8k"
    - model_pattern: "qwen-*"
      provider: qwen
      endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1"
      api_key_secret: "qwen_api_key"
    - model_pattern: "MiniMax-*"
      provider: minimax
      endpoint: "https://api.minimax.io/anthropic"
      api_key_secret: "minimax"
    - model_pattern: "*"
      provider: anthropic
      endpoint: "https://api.anthropic.com/v1"
      api_key_secret: "anthropic_api_key"
```

Routes evaluate in order; first `model_pattern` match wins. Patterns support `*` wildcard prefix/suffix matching. `extra_headers` map is available per route for provider-specific requirements.

**Cowork credential-helper auth path.** Claude for Work clients configure a "Credential helper script" that prints a bearer token on stdout; core now ships the helper binary plus the matching gateway endpoints that exchange a lower-privilege credential for a short-lived JWT carrying canonical identity headers. Endpoints mounted under `/v1/gateway/auth/cowork/` when `gateway.enabled: true`:

- `POST /pat` — `Authorization: Bearer <pat>` → verifies via `ApiKeyService`, loads the user via `OAuthRepository::get_authenticated_user`, returns `{token, ttl, headers}` with a fresh JWT and the canonical header map.
- `POST /session` — `501` (dashboard-cookie exchange not yet wired).
- `POST /mtls` — `501` (device-cert exchange not yet wired).
- `GET /capabilities` — `{"modes":["pat"]}`; probes advertise which exchange modes the deployment accepts.

The JWT-assembly + header map live in `systemprompt_oauth::services::cowork` (`issue_cowork_access`, `issue_cowork_access_with`, `CoworkAuthResult`). Response headers use core's canonical constants from `systemprompt_identifiers::headers::*` (`x-user-id`, `x-session-id`, `x-trace-id`, `x-client-id`, `x-tenant-id`, `x-policy-version`, `x-call-source`) so Cowork merges them into every subsequent `/v1/messages` call and the gateway middleware reads real identity on every request.

**`systemprompt-cowork` credential helper + sync agent.** Standalone crate at `bin/cowork/` (excluded from the workspace so it does not compile during `cargo build --workspace` and does not land in the `systemprompt` crates.io package). Dependency footprint is deliberately minimal (`ureq` + `rustls` + `serde` + `toml` + `ed25519-dalek`) — no `tokio`, `sqlx`, or `axum`.

- **Progressive capability ladder**: mTLS → dashboard session → PAT. First provider that returns a token wins; absent providers return `NotConfigured` and the chain falls through. No user-facing "pick a mode" step.
- **Providers** (`src/providers/{mtls,session,pat}.rs`) share a single `AuthProvider` trait returning `Result<HelperOutput, AuthError>` where `AuthError::NotConfigured` silently advances the chain.
- **Config**: TOML at `~/.config/systemprompt/systemprompt-cowork.toml` (or `$SP_COWORK_CONFIG`). All sections optional — absent sections mean the provider is skipped. Dev overrides: `$SP_COWORK_GATEWAY_URL`, `$SP_COWORK_PAT`, `$SP_COWORK_DEVICE_CERT`, `$SP_COWORK_USER_ASSERTION`.
- **Cache**: signed JWT + expiry written to the OS cache dir with mode `0600` on unix. Cached token is emitted directly if valid; only on cache miss does the probe chain run.
- **Stdout contract**: exactly one JSON object matching `{token, ttl, headers}` — Anthropic's `inferenceCredentialHelper` format. All diagnostics go to stderr. Exit 0 on success, non-zero on failure.
- **Sync commands**: `install`, `sync`, `validate`, `uninstall` manage the Cowork `org-plugins/` mount (macOS `/Library/Application Support/Claude/org-plugins/`, Windows `C:\ProgramData\Claude\org-plugins\`, Linux `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/`) — pulling signed plugin manifests and managed MCP allowlists from the gateway.
- **Release cadence**: tagged `cowork-v*`; binaries published manually via `cargo-zigbuild` + `gh release create`. v0.2.0 at [releases/cowork-v0.2.0](https://github.com/systempromptio/systemprompt-core/releases/tag/cowork-v0.2.0) ships Linux x86_64 and Windows x86_64 (mingw). macOS builds require a Mac host (Apple's `Security` / `CoreFoundation` frameworks can't cross-compile from Linux).
- **Build targets**: `just build-cowork [target]` and `just build-cowork-all` for local compilation.

**Gateway provider registry — extensions can register custom upstreams.** `GatewayProvider` is no longer a closed enum; `GatewayRoute.provider` is a free-form string tag resolved at dispatch time against a registry built at startup. Extension crates register new providers with:

```rust
inventory::submit! {
    systemprompt_api::services::gateway::GatewayUpstreamRegistration {
        tag: "my-provider",
        factory: || std::sync::Arc::new(MyUpstream),
    }
}
```

The new `GatewayUpstream` trait (`async fn proxy(&self, ctx: UpstreamCtx<'_>)`) is the single integration seam. Built-in tags seeded automatically: `anthropic`, `minimax`, `openai`, `moonshot`, `qwen`. Extension-registered tags may shadow built-ins (logged as a warning).

**MiniMax provider.** MiniMax ships an Anthropic-compatible endpoint at `https://api.minimax.io/anthropic`, so the new `minimax` tag reuses the Anthropic-compatible upstream verbatim — streaming, tool use, and `thinking` blocks pass through untouched. The `api_key_secret` resolves through `Secrets.custom`, so no changes to the secrets schema are required.

**New typed identifiers and constants.** `ClientId::cowork()` returns `sp_cowork` (first-party via the `sp_` prefix rule). `SessionSource::Cowork` variant with `SessionSource::from_client_id("sp_cowork") → Cowork`. `systemprompt_identifiers::PolicyVersion` newtype with `PolicyVersion::unversioned()` constructor. New canonical header constants `systemprompt_identifiers::headers::TENANT_ID` and `POLICY_VERSION` alongside the existing `USER_ID`, `SESSION_ID`, `TRACE_ID`, `CLIENT_ID` family. `JwtContextExtractor::extract_for_gateway(jwt_token: &JwtToken)` accepts a typed `JwtToken` (not `&str`), validates it, and returns a `RequestContext`. `ApiPaths::GATEWAY_BASE` constant is `/v1`.

**Changed.** Gateway dispatch rewritten around the registry — `GatewayService::dispatch` is now a thin shim: resolve route → resolve API key → look up the registered upstream → hand off to `upstream.proxy(ctx)`. The old hard-coded `match route.provider { ... }` is gone. The `GatewayProvider` enum (and its `is_openai_compatible()` / `as_str()` methods) have been removed; `GatewayRoute.provider` is a `String`. Anthropic-passthrough and OpenAI-compatible behaviours are preserved — their bodies were moved verbatim into `AnthropicCompatibleUpstream` and `OpenAiCompatibleUpstream` in the new `upstream.rs`. Unknown provider tags fail fast with `Gateway provider 'xxx' is not registered`. Analytics: `event_data` column on `analytics_events` changed to `JSONB` (was `TEXT`); added `utm_content` and `utm_term` UTM parameter columns; conversion event definitions broadened to cover subscription starts, trial activations, and feature adoptions.

Full changelog: [`CHANGELOG.md`](CHANGELOG.md).

---

## Cowork — install the credential helper

The `systemprompt-cowork` binary is Claude for Work's "Credential helper script". It exchanges a PAT (or, in a future release, a dashboard session or device certificate) for a short-lived JWT + canonical identity headers, then prints one JSON object to stdout that Claude Desktop merges into every `/v1/messages` request to the gateway.

Current release: **[cowork-v0.2.0](https://github.com/systempromptio/systemprompt-core/releases/tag/cowork-v0.2.0)** — Linux x86_64 and Windows x86_64 (mingw ABI). macOS pending a Mac-hosted build.

### 1. Download the binary

**Linux x86_64**

```bash
curl -fsSL -o /usr/local/bin/systemprompt-cowork \
  https://github.com/systempromptio/systemprompt-core/releases/download/cowork-v0.2.0/systemprompt-cowork-x86_64-unknown-linux-gnu
chmod +x /usr/local/bin/systemprompt-cowork
# verify
curl -fsSL https://github.com/systempromptio/systemprompt-core/releases/download/cowork-v0.2.0/systemprompt-cowork-x86_64-unknown-linux-gnu.sha256 \
  | sha256sum -c --ignore-missing
```

**Windows x86_64** — PowerShell as Administrator:

```powershell
$dir = "C:\Program Files\systemprompt"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Invoke-WebRequest `
  -Uri "https://github.com/systempromptio/systemprompt-core/releases/download/cowork-v0.2.0/systemprompt-cowork-x86_64-pc-windows-gnu.exe" `
  -OutFile "$dir\systemprompt-cowork.exe"
# (optional) add to PATH for current user
[Environment]::SetEnvironmentVariable("PATH", "$env:PATH;$dir", "User")
```

**macOS (any arch)** — build locally until a Mac-hosted release is published:

```bash
git clone https://github.com/systempromptio/systemprompt-core.git
cd systemprompt-core
cargo build --manifest-path bin/cowork/Cargo.toml --release \
  --target "$(rustc -vV | awk '/host:/ {print $2}')"
sudo install -m 755 \
  "bin/cowork/target/$(rustc -vV | awk '/host:/ {print $2}')/release/systemprompt-cowork" \
  /usr/local/bin/
```

### 2. Configure

Write `~/.config/systemprompt/systemprompt-cowork.toml` (Linux/macOS) or `%APPDATA%\systemprompt\systemprompt-cowork.toml` (Windows):

```toml
[gateway]
url = "https://your-systemprompt-host"   # or http://localhost:8080 for local trial

[pat]
token = "sp-live-your-personal-access-token-here"
```

Issue a PAT from your systemprompt instance with `systemprompt admin users pat issue <user-id> --name cowork-laptop`.

The helper silently skips any provider whose section is absent. Dev overrides (no config file needed): `SP_COWORK_GATEWAY_URL`, `SP_COWORK_PAT`.

### 3. Validate the helper runs

```bash
systemprompt-cowork                    # prints one JSON {token, ttl, headers}
systemprompt-cowork --check            # exits 0 if a token can be issued
```

Diagnostics go to stderr; stdout is strictly the Anthropic `inferenceCredentialHelper` JSON contract.

### 4. Wire into Claude for Work

In Claude Desktop's Enterprise settings (or your fleet MDM profile):

- **Inference credential helper script**: `/usr/local/bin/systemprompt-cowork` (or the Windows path).
- **API base URL** (`api_external_url`): `https://your-systemprompt-host`.

Claude Desktop will now invoke the helper on every request, pick up the JWT, and flow `POST /v1/messages` through your gateway. Every request lands a row in `ai_requests` with `user_id`, `tenant_id`, `session_id`, `trace_id`, tokens, cost, latency — see the [governance spine in v0.3.0](#whats-new-in-v030).

### 5. (Optional) Install the `org-plugins/` sync agent

The same binary manages Cowork's plugin / managed-MCP mount:

```bash
systemprompt-cowork install     # install the launchd / scheduled task
systemprompt-cowork sync        # pull signed plugin manifest + allowlist now
systemprompt-cowork validate    # verify ed25519 signature on the manifest
systemprompt-cowork uninstall   # remove
```

Mount locations: `/Library/Application Support/Claude/org-plugins/` (macOS), `C:\ProgramData\Claude\org-plugins\` (Windows), `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/` (Linux).

---

## Capabilities

**Every tool call governed.** Synchronous evaluation before execution, not after. Four layers of enforcement in the request path: **scope check → secret detection → blocklist → rate limit**. Deny reasons are structured and auditable. Single-digit milliseconds overhead. No sidecar. No proxy. Compliance that survives an audit: **SOC 2 Type II**, **ISO 27001**, **HIPAA**, **OWASP Top 10 for Agentic Applications**.

**Secrets never touch inference** — the agent calls the tool, the MCP service injects the credential server-side, the LLM never sees it. Per-user key hierarchy encrypted with **ChaCha20-Poly1305**. Every tool call produces a **five-point audit trace**: *Identity → Agent Context → Permissions → Tool Execution → Result*. Everything linked by `trace_id`. Structured JSON events for Splunk, ELK, Datadog, Sumo Logic. Cost tracking in microdollars by model, agent, and department.

**Where in the code**

| Concern | File |
|---|---|
| Scope / RBAC middleware | [`crates/domain/mcp/src/middleware/rbac.rs`](crates/domain/mcp/src/middleware/rbac.rs) |
| Secret detection / scanner | [`crates/infra/security/src/services/scanner.rs`](crates/infra/security/src/services/scanner.rs) |
| Blocklist rules | [`crates/infra/security/src/services/`](crates/infra/security/src/services/) |
| Rate limit middleware (`tower_governor`) | [`crates/infra/security/src/`](crates/infra/security/src/) |
| Audit queries | [`crates/infra/logging/src/trace/audit_queries.rs`](crates/infra/logging/src/trace/audit_queries.rs) |
| Event broadcasters | [`crates/infra/events/src/services/broadcaster.rs`](crates/infra/events/src/services/broadcaster.rs) |
| Secret storage (ChaCha20-Poly1305) | [`crates/infra/security/src/`](crates/infra/security/src/) |
| Typed IDs (`TraceId`, `ContextId`, `TaskId` …) | [`crates/shared/identifiers/src/lib.rs`](crates/shared/identifiers/src/lib.rs) |

**MCP** ([`crates/domain/mcp`](crates/domain/mcp)) is implemented natively — not proxied. Per-server OAuth2, scoped tool exposure, central registry with health monitoring, end-to-end access logs. Works with Claude Code, Claude Desktop, ChatGPT, Cursor, and any other MCP-compatible client.

| Concern | File |
|---|---|
| Orchestrator | [`crates/domain/mcp/src/services/orchestrator/mod.rs`](crates/domain/mcp/src/services/orchestrator/mod.rs) |
| Network / port management / proxy | [`crates/domain/mcp/src/services/network/mod.rs`](crates/domain/mcp/src/services/network/mod.rs) |
| RBAC middleware | [`crates/domain/mcp/src/middleware/rbac.rs`](crates/domain/mcp/src/middleware/rbac.rs) |

```json
{
  "mcpServers": {
    "my-server": {
      "url": "https://my-tenant.systemprompt.io/api/v1/mcp/my-server/mcp",
      "transport": "streamable-http"
    }
  }
}
```

**Agent-to-Agent** ([`crates/domain/agent`](crates/domain/agent)) ships a standalone A2A server with streaming, a JSON-RPC protocol model, and `.well-known` discovery endpoints.

| Concern | File |
|---|---|
| Standalone A2A server | [`crates/domain/agent/src/services/a2a_server/mod.rs`](crates/domain/agent/src/services/a2a_server/mod.rs) |
| Streaming | [`crates/domain/agent/src/services/a2a_server/streaming/mod.rs`](crates/domain/agent/src/services/a2a_server/streaming/mod.rs) |
| Protocol models (`Message`, `Task`, `TaskState`) | [`crates/domain/agent/src/models/a2a/protocol/mod.rs`](crates/domain/agent/src/models/a2a/protocol/mod.rs) |

**Discovery API**

| Endpoint | Description |
|---|---|
| `/.well-known/agent-card.json` | Default agent card |
| `/.well-known/agent-cards` | List all available agents |
| `/.well-known/agent-cards/{name}` | Specific agent card |
| `/api/v1/agents/registry` | Full agent registry with status |
| `/api/v1/mcp/registry` | All MCP servers with endpoints |

- [Governance Pipeline](https://systemprompt.io/features/governance-pipeline)
- [Secrets Management](https://systemprompt.io/features/secrets-management)
- [MCP Governance](https://systemprompt.io/features/mcp-governance)
- [Analytics & Observability](https://systemprompt.io/features/analytics-and-observability)
- [Closed-Loop Agents](https://systemprompt.io/features/closed-loop-agents)
- [Compliance](https://systemprompt.io/features/compliance)

---

## Quick Start

**Evaluation path** — you get 40+ runnable demos:

```bash
gh repo create my-eval --template systempromptio/systemprompt-template --clone
cd my-eval
just build
just setup-local <anthropic-or-openai-or-gemini-key>
just start
```

Open **http://localhost:8080**, point Claude Code / Claude Desktop at it, and walk through [`demo/`](https://github.com/systempromptio/systemprompt-template/tree/main/demo). Prerequisites: Rust 1.75+, [`just`](https://just.systems), Docker, `jq`, `yq`, ports `8080` and `5432` free.

**Library path** — add the facade to your own Rust workspace:

```toml
[dependencies]
systemprompt = { version = "0.3.0", features = ["full"] }
```

See [Extensions (technical)](#extensions-technical) for the compile-time plugin model.

---

<details>
<summary><strong>Infrastructure</strong></summary>

<br>

**One binary. One database. Deploys anywhere.** The same surface local and remote. Config-as-code: agents, MCP servers, skills, AI providers, content, scheduler jobs, and web theme all live as YAML or Markdown under `services/`. Built on open standards: **MCP** (Model Context Protocol), **A2A** (Agent-to-Agent), **OAuth2/OIDC** with PKCE, **WebAuthn**.

**Where in the code**

| Concern | File |
|---|---|
| Bootstrap sequence | `ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config → AppContext` |
| AppContext wiring | [`crates/app/runtime/src/context.rs`](crates/app/runtime/src/context.rs) · [`builder.rs`](crates/app/runtime/src/builder.rs) |
| Provider traits (`LlmProvider`, `ToolProvider`, …) | [`crates/shared/provider-contracts/src/lib.rs`](crates/shared/provider-contracts/src/lib.rs) |
| CLI entry point (8 domains) | [`crates/entry/cli/src/commands/`](crates/entry/cli/src/commands/) |

One binary, eight domains. Every command is discoverable — `systemprompt <domain> --help` works everywhere.

| Domain | Source | Purpose |
|---|---|---|
| `core` | [`crates/entry/cli/src/commands/core/`](crates/entry/cli/src/commands/core/) | Skills, content, files, contexts, plugins, hooks, artifacts |
| `infra` | [`crates/entry/cli/src/commands/infrastructure/`](crates/entry/cli/src/commands/infrastructure/) | Services, database, jobs, logs |
| `admin` | [`crates/entry/cli/src/commands/admin/`](crates/entry/cli/src/commands/admin/) | Users, agents, config, setup, session, rate limits |
| `cloud` | [`crates/entry/cli/src/commands/cloud/`](crates/entry/cli/src/commands/cloud/) | Auth, deploy, sync, secrets, tenant, domain |
| `analytics` | [`crates/entry/cli/src/commands/analytics/`](crates/entry/cli/src/commands/analytics/) | Overview, conversations, agents, tools, requests, sessions, content, traffic, costs |
| `web` | [`crates/entry/cli/src/commands/web/`](crates/entry/cli/src/commands/web/) | Content types, templates, assets, sitemap, validate |
| `plugins` | [`crates/entry/cli/src/commands/plugins/`](crates/entry/cli/src/commands/plugins/) | Extensions, MCP servers, capabilities |
| `build` | [`crates/entry/cli/src/commands/build/`](crates/entry/cli/src/commands/build/) | Build core workspace and MCP extensions |

- [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)
- [No Vendor Lock-In](https://systemprompt.io/features/no-vendor-lock-in)

</details>

<details>
<summary><strong>Integrations</strong></summary>

<br>

**Provider-agnostic. Protocol-native. Fully extensible.** Provider-agnostic by trait, not by adapter — swap **Anthropic / OpenAI / Gemini** at the profile level.

- [Any AI Agent](https://systemprompt.io/features/any-ai-agent)
- [Extensible Architecture](https://systemprompt.io/features/extensible-architecture)
- [Skill Marketplace](https://systemprompt.io/features/skill-marketplace)

</details>

<details>
<summary><strong>Architecture</strong></summary>

<br>

A 30-crate Rust workspace that compiles into a single ~50 MB binary. Dependencies flow downward only — no circular references.

```
┌─────────────────────────────────────────────────────────────────────┐
│  ENTRY      api · cli                                               │
├─────────────────────────────────────────────────────────────────────┤
│  APP        runtime · scheduler · generator · sync                  │
├─────────────────────────────────────────────────────────────────────┤
│  DOMAIN     agent · ai · analytics · content · files · mcp ·        │
│             oauth · templates · users                               │
├─────────────────────────────────────────────────────────────────────┤
│  INFRA      cloud · config · database · events · loader ·           │
│             logging · security                                      │
├─────────────────────────────────────────────────────────────────────┤
│  SHARED     identifiers · provider-contracts · traits ·             │
│             extension · models · client · template-provider        │
└─────────────────────────────────────────────────────────────────────┘
```

All 30 crates publish on crates.io at matching workspace versions. Domain crates communicate via traits and the event bus, not direct dependencies. Database-touching crates ship a per-crate `.sqlx/` query cache (committed) so downstream consumers compile offline.

| Layer | Crates |
|---|---|
| Shared | [`systemprompt-identifiers`](https://docs.rs/systemprompt-identifiers) · [`systemprompt-provider-contracts`](https://docs.rs/systemprompt-provider-contracts) · [`systemprompt-traits`](https://docs.rs/systemprompt-traits) · [`systemprompt-extension`](https://docs.rs/systemprompt-extension) · [`systemprompt-models`](https://docs.rs/systemprompt-models) · [`systemprompt-client`](https://docs.rs/systemprompt-client) · [`systemprompt-template-provider`](https://docs.rs/systemprompt-template-provider) |
| Infra | [`systemprompt-database`](https://docs.rs/systemprompt-database) · [`systemprompt-logging`](https://docs.rs/systemprompt-logging) · [`systemprompt-events`](https://docs.rs/systemprompt-events) · [`systemprompt-security`](https://docs.rs/systemprompt-security) · [`systemprompt-loader`](https://docs.rs/systemprompt-loader) · [`systemprompt-config`](https://docs.rs/systemprompt-config) · [`systemprompt-cloud`](https://docs.rs/systemprompt-cloud) |
| Domain | [`systemprompt-analytics`](https://docs.rs/systemprompt-analytics) · [`systemprompt-users`](https://docs.rs/systemprompt-users) · [`systemprompt-files`](https://docs.rs/systemprompt-files) · [`systemprompt-templates`](https://docs.rs/systemprompt-templates) · [`systemprompt-content`](https://docs.rs/systemprompt-content) · [`systemprompt-ai`](https://docs.rs/systemprompt-ai) · [`systemprompt-oauth`](https://docs.rs/systemprompt-oauth) · [`systemprompt-mcp`](https://docs.rs/systemprompt-mcp) · [`systemprompt-agent`](https://docs.rs/systemprompt-agent) |
| App | [`systemprompt-runtime`](https://docs.rs/systemprompt-runtime) · [`systemprompt-scheduler`](https://docs.rs/systemprompt-scheduler) · [`systemprompt-generator`](https://docs.rs/systemprompt-generator) · [`systemprompt-sync`](https://docs.rs/systemprompt-sync) |
| Entry | [`systemprompt-api`](https://docs.rs/systemprompt-api) · [`systemprompt-cli`](https://docs.rs/systemprompt-cli) |
| Facade | [`systemprompt`](https://docs.rs/systemprompt) |

</details>

<details>
<summary><strong>Extensions (technical)</strong></summary>

<br>

Extensions are discovered at **compile time** via the [`inventory`](https://crates.io/crates/inventory) crate — no runtime plugin loading, no `dlopen`. Your code compiles straight into your binary. Typed traits cover the full surface:

| Trait | File | Purpose |
|---|---|---|
| `Extension` | [`crates/shared/extension/src/traits.rs`](crates/shared/extension/src/traits.rs) | Identity, version, dependency metadata |
| `SchemaExtensionTyped` | [`crates/shared/extension/src/typed/schema.rs`](crates/shared/extension/src/typed/schema.rs) | DDL + migrations via `include_str!()` |
| `ApiExtensionTyped` · `ApiExtensionTypedDyn` | [`crates/shared/extension/src/typed/api.rs`](crates/shared/extension/src/typed/api.rs) | Axum route handlers |
| `JobExtensionTyped` | [`crates/shared/extension/src/typed/job.rs`](crates/shared/extension/src/typed/job.rs) | Scheduled and background jobs |
| `ProviderExtensionTyped` | [`crates/shared/extension/src/typed/provider.rs`](crates/shared/extension/src/typed/provider.rs) | Custom LLM / tool / data providers |
| `ConfigExtensionTyped` | [`crates/shared/extension/src/typed/config.rs`](crates/shared/extension/src/typed/config.rs) | Startup config validation |

Registration is a single macro — `register_extension!` lives in [`crates/shared/extension/src/traits.rs`](crates/shared/extension/src/traits.rs) and wraps `inventory::submit!`. Discovery goes through [`ExtensionBuilder<R>`](crates/shared/extension/src/builder.rs) and `TypedExtensionRegistry`.

```toml
[dependencies]
systemprompt = { version = "0.3.0", features = ["full"] }
```

```rust
use systemprompt::extension::prelude::*;

struct MyExtension;

impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new("my-extension", env!("CARGO_PKG_VERSION"))
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            "my_extension",
            include_str!("../schema/001_init.sql"),
        )]
    }

    fn router(&self) -> Option<ExtensionRouter> { None }
}

register_extension!(MyExtension);
```

</details>

<details>
<summary><strong>Typed identifiers</strong></summary>

<br>

**Zero raw-String IDs.** Every identifier that crosses a boundary is a newtype in [`crates/shared/identifiers`](crates/shared/identifiers/src/lib.rs) — the compiler prevents passing a `UserId` where an `AgentId` is expected.

`UserId` · `SessionId` · `TraceId` · `ContextId` · `TaskId` · `AgentId` · `TenantId` · `McpServerId` · `McpExecutionId` · `AiRequestId` · `PluginId` · `SkillId` · `ArtifactId` · `FileId` · `ContentId` · `MessageId` · `TokenId` · `ClientId` · `RoleId` · `ProfileName` · `Email` · `ValidatedUrl` · `ValidatedFilePath` · `PolicyVersion`

</details>

<details>
<summary><strong>Database & repositories</strong></summary>

<br>

Services call repositories, repositories issue SQL. All queries go through **compile-time verified macros** — `sqlx::query!()`, `sqlx::query_as!()`, `sqlx::query_scalar!()`. No unverified `sqlx::query()`.

DDL lives in `{crate}/schema/*.sql` and is embedded with `include_str!()` from `extension.rs`. The generic entity/repository traits live in [`crates/infra/database/src/repository/entity.rs`](crates/infra/database/src/repository/entity.rs) (`Entity`, `GenericRepository<E>`).

```rust
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

pub struct UserRepository { pool: DbPool }

impl UserRepository {
    pub async fn find_by_id(&self, id: &UserId) -> Result<Option<User>> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id.as_str())
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(Into::into)
    }
}
```

</details>

<details>
<summary><strong>Facade crate & feature flags</strong></summary>

<br>

Pull in only what you need through the `systemprompt` facade.

| Feature | Includes |
|---|---|
| `core` *(default)* | traits · models · identifiers · extension · template-provider |
| `database` | SQLx-backed `DbPool` |
| `api` | HTTP server, runtime, Axum (requires `core` + `database`) |
| `cli` | CLI entry point |
| `runtime` | Extension runtime builder (requires `cli`) |
| `mcp` | `rmcp` macros |
| `sync` | Cloud synchronization |
| `cloud` | Cloud API client, credentials, OAuth |
| `test-utils` | Credential fixtures (requires `cloud`) |
| `full` | Everything: API + MCP + sync + cloud + CLI + all domain crates |

```toml
# Embedded library usage
systemprompt = { version = "0.3.0", features = ["core", "database"] }

# Building a product binary
systemprompt = { version = "0.3.0", features = ["full"] }
```

```rust
use systemprompt::prelude::*;
use systemprompt::database::DbPool;
```

</details>

<details>
<summary><strong>Performance</strong></summary>

<br>

Sub-5 ms governance overhead, benchmarked. Each request performs JWT validation, scope resolution, three rule evaluations, and an async database write.

- **p50 < 5 ms**
- **p99 < 12 ms**
- **200 concurrent governance requests**
- Zero GC pauses — hundreds of concurrent developers on a single instance

Numbers measured on the author's laptop. Reproduce with `./demo/performance/02-benchmark.sh` in the template. Full results and a live load test: [systemprompt.io/features/demo](https://systemprompt.io/features/demo).

</details>

---

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. **Production use requires a commercial license.** Each version converts to Apache 2.0 four years after publication.

See [LICENSE](LICENSE) for the full terms. Licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt)** · **[docs.rs](https://docs.rs/systemprompt)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Own how your organization uses AI. Every interaction governed and provable.</sub>

</div>
