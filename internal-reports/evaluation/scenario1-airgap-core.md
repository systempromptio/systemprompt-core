# Bucket 1 — Scenario 1 (Air-Gapped), systemprompt-core

> One of four buckets in the customer deployment evaluation.
> Master plan: scenario 1 (closed/air-gapped) + scenario 2 (scaled/distributed),
> each split into a `core` and a `template` bucket. This document covers the
> **core-repo** work for the **air-gapped** scenario.

## Context

The customer is evaluating systemprompt.io for a sensitive-infrastructure deployment: a
fully network-isolated, air-gapped system whose inference is served by an **internal**
OpenAI/Anthropic-compatible endpoint. No external egress is permitted. Before the
customer can run this on real hardware we must be able to:

1. Reproduce the deployment shape with no external dependencies.
2. Load-test it and produce verified, re-runnable metrics.
3. Prove the engine makes **zero** outbound connections beyond the configured endpoint.

This bucket delivers the **reusable core-repo tooling** for that. The deployment
topology and the egress proof itself live in Bucket 2 (template repo); this bucket
gives Bucket 2 the mock inference server, the load scenarios, the air-gap profile, and
the egress inventory it consumes.

## Verified facts (from codebase exploration)

**Inference path.** A client calls `POST /v1/messages` (Anthropic wire format)
or `POST /v1/responses` (OpenAI). Handler:
`crates/entry/api/src/routes/gateway/messages/mod.rs::handle()`. The pipeline runs in
`crates/entry/api/src/services/gateway/service/mod.rs::dispatch()`:

1. Auth — `routes/gateway/messages/auth.rs::authenticate()` (JWT or `sp_` API key).
2. Policy resolve — `services/gateway/policy.rs::PolicyResolver::resolve()`, 60 s cache.
3. Scope check — `policy.model_allowed(&request.model)`.
4. Quota pre-check/reserve — `services/gateway/quota.rs::precheck_and_reserve()`,
   DB-backed (`AiQuotaBucketRepository`).
5. Request safety scan — `services/gateway/service/finalize.rs::run_request_safety_scan()`
   (heuristic, non-blocking).
6. Outbound adapter — `services/gateway/protocol/outbound/{anthropic,openai_chat,
   openai_responses}/mod.rs`.

**Routing is config-driven** via `gateway.routes[]` in `profile.yaml`: each route has
`model_pattern`, `provider`, `endpoint`, `api_key_secret`, `upstream_model`. The
Anthropic adapter posts to `{endpoint}/messages`; the OpenAI-chat adapter to
`{endpoint}/chat/completions`. There is **no hard-coded inference URL** — pointing at an
internal endpoint is pure configuration.

**Air-gap viability.** No license or update checks anywhere. Secrets are customer-
managed; config is static YAML. No `build.rs` makes outbound calls (verified by grep —
the 11 build scripts only emit `cargo:` directives and discover migrations). Test
fixtures under `crates/tests/` are excluded from this inventory — they are not
production code and never ship.

The **egress inventory** below is the finalised result of a full repo sweep for
`reqwest::Client::new` / `Client::builder` / `ClientBuilder`. It is the contract
Bucket 2's `01-egress-assert.sh` verifies. 24 production egress points exist, grouped
by category. Columns: trigger, target-host source, and how to keep each silent in an
air-gapped deployment.

**Gateway outbound** — the intended inference path; each must point at the internal
endpoint via `gateway.routes[]` config.

| # | File:line | Trigger | Target host source | Air-gap concern |
|---|-----------|---------|--------------------|-----------------|
| 1 | `crates/entry/api/src/services/gateway/protocol/outbound/anthropic/mod.rs:25` | per-request | request-context (`ctx.route.endpoint`) | intended — point at internal endpoint |
| 2 | `crates/entry/api/src/services/gateway/protocol/outbound/openai_chat/mod.rs:27` | per-request | request-context (`ctx.route.endpoint`) | intended — point at internal endpoint |
| 3 | `crates/entry/api/src/services/gateway/protocol/outbound/openai_responses/mod.rs:25` | per-request | request-context (`ctx.route.endpoint`) | intended — point at internal endpoint |

**Proxy / health** — relay and probe clients; must only ever target internal hosts.

| # | File:line | Trigger | Target host source | Air-gap concern |
|---|-----------|---------|--------------------|-----------------|
| 4 | `crates/entry/api/src/services/proxy/client.rs:19` | startup | request-context (proxied URL) | must target only internal hosts |
| 5 | `crates/entry/api/src/services/health/checker.rs:36` | on-demand | config (`self.url`) | must target only internal hosts |

**Cloud** — cloud control-plane clients; must stay unused in an air-gapped deployment.

| # | File:line | Trigger | Target host source | Air-gap concern |
|---|-----------|---------|--------------------|-----------------|
| 6 | `crates/infra/cloud/src/oauth/client.rs:102` | on-demand | config (`api_url`) | must stay unused air-gapped |
| 7 | `crates/infra/cloud/src/credentials.rs:83` | on-demand | config (`GET {api_url}/api/v1/auth/me`) | must stay unused air-gapped |

**MCP relay** — MCP transport and proxy; must only reach internal MCP servers.

| # | File:line | Trigger | Target host source | Air-gap concern |
|---|-----------|---------|--------------------|-----------------|
| 8 | `crates/domain/mcp/src/services/network/proxy.rs:28` | per-request | dynamic (`target_url`) | internal MCP only |
| 9 | `crates/domain/mcp/src/services/client/http_client_with_context.rs:30` | startup (transport init) | dynamic (MCP server URI) | internal MCP only |

**Non-gateway AI providers** — the direct AI service path (agent/MCP-internal LLM
calls); use an internal endpoint or leave unused.

| # | File:line | Trigger | Target host source | Air-gap concern |
|---|-----------|---------|--------------------|-----------------|
| 10 | `crates/domain/ai/src/services/providers/http_client.rs:28` | startup (shared builder) | config (provider base URL) | internal endpoint or unused |
| 11 | `crates/domain/ai/src/services/providers/anthropic/provider.rs:17` | startup (via `build_client`) | config (provider base URL) | internal endpoint or unused |
| 12 | `crates/domain/ai/src/services/providers/openai/provider.rs` | startup (via `build_client`) | config (provider base URL) | internal endpoint or unused |
| 13 | `crates/domain/ai/src/services/providers/gemini/provider.rs` | startup (via `build_client`) | config (provider base URL) | internal endpoint or unused |
| 14 | `crates/domain/ai/src/services/providers/openai_images.rs:26` | startup | config (provider base URL) | internal endpoint or unused |
| 15 | `crates/domain/ai/src/services/providers/gemini_images.rs:35` | startup | config (provider base URL) | internal endpoint or unused |

**Bridge** — local bridge ↔ gateway traffic; internal by design.

| # | File:line | Trigger | Target host source | Air-gap concern |
|---|-----------|---------|--------------------|-----------------|
| 16 | `bin/bridge/src/proxy/server.rs:97` | startup (pool) | config (`gateway_base`) | intended — bridge↔gateway internal traffic |
| 17 | `bin/bridge/src/gateway/mod.rs:28` | startup (lazy-static pool) | config (`gateway_base`) | intended — bridge↔gateway internal traffic |
| 18 | `bin/bridge/src/proxy/heartbeat.rs:89` | scheduled (~30 s) | config (`POST {gateway_base}/v1/bridge/heartbeat`) | intended — internal heartbeat |
| 19 | `bin/bridge/src/proxy/forward.rs:118` | per-request | config (`gateway_base` route) | intended — internal forwarding |

**Other** — webhooks, generic API client, sync, and CLI.

| # | File:line | Trigger | Target host source | Air-gap concern |
|---|-----------|---------|--------------------|-----------------|
| 20 | `crates/infra/security/src/authz/hook.rs:109` | on-demand (WebhookHook) | config (webhook URL) | block at network level / leave unconfigured |
| 21 | `crates/shared/client/src/client.rs:26` | startup | config (`base_url`) | must target only internal hosts |
| 22 | `crates/app/sync/src/api_client/mod.rs:52` | on-demand (push/pull/deploy) | config (cloud API URL) | must stay unused air-gapped |
| 23 | `crates/entry/cli/src/session/api.rs:23` | CLI | config (`POST {api_url}/api/v1/core/oauth/session`) | must target only internal hosts |
| 24 | `crates/entry/cli/src/routing/remote.rs:28` | CLI (remote command exec) | config (remote API URL) | must target only internal hosts |

**Agent / CLI extras** (not counted in the 24 — agent-internal health and webhook
plumbing, plus CLI utilities). These either loop back to `localhost` or are
config-/request-gated:

- `crates/domain/agent/src/services/external_integrations/webhook/service/mod.rs:30` —
  outbound webhook delivery, config-driven endpoint; block at network level or leave
  unconfigured.
- `crates/domain/agent/src/services/a2a_server/streaming/broadcast.rs:45,97,165` and
  `crates/domain/agent/src/services/a2a_server/streaming/webhook_client.rs:58,130` —
  A2A push-notification / streaming delivery to subscriber URLs; config-driven, block
  at network level.
- `crates/domain/agent/src/services/agent_orchestration/monitor.rs:244` — A2A health
  check `GET http://localhost:{port}/.well-known/agent-card.json`; hard-coded localhost,
  never leaves the host.
- `crates/domain/agent/src/services/mcp/task_helper/completion.rs:60` — agent-side MCP
  task completion call; dynamic internal target.
- `crates/entry/cli/src/commands/core/content/verify.rs:73` — content-verify HEAD
  request; CLI-only, config-driven URL.
- `crates/entry/cli/src/commands/admin/agents/{message_streaming.rs:21,registry.rs:87}`
  — CLI clients to the local agent/API; loopback in normal use.

Optional egress to disable in config (Bucket 2 owns this): `google_search_enabled` per
provider, and cloud sync.

**Authentication.** systemprompt issues its own HS256 (HMAC-SHA256, shared-secret)
service tokens internally — `crates/infra/security/src/auth/validation.rs` accepts only
`Algorithm::HS256`. Customer IdP federation is supported today via the OAuth2
authorization-code flow, not direct asymmetric (RS256/ES256/EdDSA) JWT verification —
the latter is documented as roadmap in `documentation/security/threat-model.md`. For an
air-gapped deployment any IdP used via OAuth2 must sit **inside** the network; an
out-of-network IdP would be egress.

**Existing harness.** `crates/tests/loadtest/` is a single-process generator. Scenarios
are `pub async fn run(client, base_url, token, metrics)` functions, registered by a
`match` in `src/main.rs` and listed in the `"all"` vector. Profiles (`ci`, `default`)
are hardcoded structs in `src/config.rs` selected by a `match` in `main.rs`. Output is
**text-only** (`src/metrics.rs::Report::print`). Auth: `src/auth.rs::acquire_token()`
runs `systemprompt admin session login --token-only` with `SYSTEMPROMPT_PROFILE` set,
scraping the `eyJ…` JWT from stdout. The harness expects an already-running server at
`--base-url`.

**Test workspace.** `crates/tests/Cargo.toml` is a separate workspace (`members` list +
`resolver = "2"`). Workspace deps available: `axum 0.8`, `tokio 1.49`, `reqwest 0.12`
(json/stream/rustls-tls), `sqlx 0.8`, `serde`, `serde_json`, `clap`. A new member crate
is a directory under `crates/tests/<name>/` added to `members`.

## Work items

### 1. New crate `crates/tests/mock-inference/`
A standalone axum binary that stands in for the customer's internal inference endpoint,
so the air-gapped scenario is fully reproducible with zero model cost and zero external
calls.

- `Cargo.toml`: `publish = false`, `edition = "2024"`, deps `axum`, `tokio`, `serde`,
  `serde_json`, `clap` (all workspace). Add `"mock-inference"` to `crates/tests/Cargo.toml`
  `members`.
- Routes:
  - `POST /messages` — Anthropic Messages wire format. Response shape must satisfy
    `services/gateway/protocol/outbound/anthropic/response.rs::parse_response()`.
  - `POST /chat/completions` — OpenAI Chat format, satisfying `openai_chat/response.rs`.
  - Both: non-streaming JSON **and** SSE streaming (`stream: true`).
  - `GET /health` → 200.
- Behaviour flags (clap): `--port`, `--latency-ms` (fixed or `min:max` jitter),
  `--fail-rate` (0.0–1.0), `--mode` (`ok` | `timeout` | `5xx` | `slow-loris`).
- Deterministic token-usage echo (input tokens counted, output tokens fixed) so latency
  numbers are reproducible.
- Built as a binary so Bucket 2 can containerise it.

### 2. New loadtest scenario `gateway_inference.rs`
`crates/tests/loadtest/src/scenarios/gateway_inference.rs` — `pub async fn run(...)`
that POSTs an Anthropic-format body to `{base_url}/v1/messages` with the
required headers (`Authorization: Bearer`, `x-systemprompt-session-id`). Exercises the
full pipeline (auth → policy → quota → safety scan → outbound to the mock). Measures the
latency the platform adds on top of mock inference.
- Register: `pub mod gateway_inference;` in `scenarios/mod.rs`; dispatch branch in
  `main.rs`; add `"gateway-inference"` to the `"all"` vector.

### 3. New loadtest scenario `governance_only.rs`
`crates/tests/loadtest/src/scenarios/governance_only.rs` — POSTs a request whose model
is **denied by policy** (model not in `allowed_models`), so the pipeline returns at the
scope-check step with no upstream call. Isolates policy + quota evaluation cost from
inference cost. Same registration steps as item 2.

### 4. New `airgap` load profile
In `crates/tests/loadtest/src/config.rs`, add `LoadConfig::airgap(base_url, token)`:
single target, moderate concurrency stage ramp, strict thresholds — `p95_ms: 300`,
`p99_ms: 600`, `max_error_rate: 0.005`. Add `"airgap"` to the profile `match` in
`main.rs`.

### 5. JSON reporter `crates/tests/loadtest/src/reporters/json.rs`
Add `--output <text|json>` and `--out-file <path>` CLI args. `text` stays the default
(existing `metrics.rs` printer). `json` serialises per-scenario `{requests, p50, p95,
p99, error_rate, passed}` plus an aggregate to a file — needed so Bucket 2/the report
consume structured results, not scraped stdout. *Shared with Bucket 3; land here if
Bucket 1 executes first, otherwise consume it.*

### 6. Egress inventory section
The finalised egress inventory under "Verified facts" above — the 24-row table produced
by a full repo sweep for `reqwest::Client` / `ClientBuilder` across the engine,
recording for each: trigger condition, target host source, and how to keep it silent
air-gapped. This is the contract Bucket 2's `01-egress-assert.sh` verifies.

### 7. Justfile recipes
In `systemprompt-core/justfile`:
- `mock-inference *ARGS` → `cargo run --manifest-path crates/tests/mock-inference/Cargo.toml -- {{ARGS}}`
- `loadtest-airgap *ARGS` → `cargo run --manifest-path crates/tests/loadtest/Cargo.toml -- --profile airgap {{ARGS}}`

## Critical files

- **New:** `crates/tests/mock-inference/{Cargo.toml,src/main.rs,src/anthropic.rs,src/openai.rs}`
- **New:** `crates/tests/loadtest/src/scenarios/{gateway_inference.rs,governance_only.rs}`
- **New:** `crates/tests/loadtest/src/reporters/{mod.rs,json.rs}`
- **Edit:** `crates/tests/Cargo.toml` (members), `crates/tests/loadtest/src/main.rs`
  (CLI args, dispatch, profile match), `crates/tests/loadtest/src/config.rs` (airgap
  profile), `crates/tests/loadtest/src/scenarios/mod.rs`, `crates/tests/loadtest/src/metrics.rs`
- **Edit:** `systemprompt-core/justfile`

## Constraints

- Tests/harness code lives in the `crates/tests/` workspace, never inline `#[cfg(test)]`.
- Rust standards apply: `thiserror` not `anyhow` in libraries (the loadtest/mock crates
  are binaries — `anyhow` is acceptable there); typed IDs; `tracing` not `println!`
  except the loadtest's existing CLI display sink.
- No legacy/dual-path code; land new code and remove anything it replaces together.

## Verification

1. `cargo +nightly fmt --all && cargo clippy --workspace --all-targets --all-features
   -- -D warnings` clean.
2. The mock-inference and loadtest crates build in the test workspace.
3. Run `just mock-inference --port 9100`; start a local API with a `gateway.routes`
   entry pointing at `http://localhost:9100/messages`; run `just loadtest-airgap
   --scenario gateway-inference --output json --out-file /tmp/airgap.json`.
4. The JSON artifact exists and the run meets the `airgap` thresholds.
5. `governance_only` returns policy-deny responses with no mock-inference hits.

## Hand-off to Bucket 2

Bucket 2 (template) consumes: the `mock-inference` binary (containerised), the `airgap`
loadtest profile, the `gateway_inference` + `governance_only` scenarios, the JSON
reporter, and the finalised egress inventory.
