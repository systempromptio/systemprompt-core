# Changelog

## [0.3.2] - 2026-04-24

### Fixed

- **Static content route handler scoped slug lookup by `source_id`** (`crates/entry/api/src/services/static_content/static_files.rs`). `serve_static_content` extracted `(slug, source_id)` from the route matcher but discarded the source, then called `ContentRepository::get_by_slug(slug)` — a slug-only query. Any slug present in a different source (e.g. `about` as a page) caused `/guides/about`, `/documentation/about`, etc. to match a foreign record and return the "Content Not Prerendered" 500 page instead of 404. `source_id` is now threaded through `ContentPageRequest` and lookup uses `get_by_source_and_slug`.

- **Surface binary name and domain identifier on `Command::new` and `File::open` spawn errors across MCP, scheduler, sync, agent, and CLI paths.** The MCP port-manager reconciliation (`crates/domain/mcp/src/services/network/port_manager.rs`) shelled out to `lsof -ti :<port>` with bare `?` propagation. When `lsof` was missing from the runtime image, the ENOENT on `execve("lsof")` surfaced as a contextless `No such file or directory (os error 2)` and required `strace` to diagnose. Root fix is adding `lsof` to the runtime apt list, but the diagnosability gap is systemic: ~30% of `Command::new` sites discarded the binary name, args, and relevant identifier (port/pid/pattern/path) from the error path.

  Wrapped every flagged spawn site with `anyhow::Context::with_context` (or `tracing::warn!` where the return type is `Option`/`bool` and changing the signature would ripple through callers). Error messages now name the invocation (`failed to run \`lsof -ti :{port}\` for port {port}`) plus the domain identifier so operators don't have to re-derive context.

  Files touched: `crates/domain/mcp/src/services/network/port_manager.rs` (primary incident), `crates/domain/mcp/src/services/process/{pid_manager,cleanup,monitor,utils}.rs`, `crates/app/scheduler/src/services/orchestration/process_cleanup.rs` (previously silent `.ok()?` / `.is_ok_and(...)` converted to logging on failure), `crates/domain/agent/src/services/agent_orchestration/{port_manager,process}.rs`, `crates/domain/agent/src/services/agent_orchestration/orchestrator/cleanup.rs`, `crates/entry/cli/src/commands/cloud/tenant/docker/database.rs` (7 `docker exec psql` sites), `crates/entry/cli/src/shared/docker.rs`, `crates/app/sync/src/crate_deploy.rs` (new `SyncError::CommandSpawnFailed` variant), `crates/app/sync/src/file_bundler.rs` (new `SyncError::FileOpenFailed` variant), `crates/entry/cli/src/commands/web/templates/show.rs`.

- **HTTP-client timeout literals scattered across ~15 sites consolidated into `systemprompt_models::net`.** Generic 30s / 10s / 5s timeouts were inlined as `Duration::from_secs(…)` literals across cloud API, sync API, CIMD fetcher, OAuth credentials verify, CLI session auth, shared `SystempromptClient`, MCP streaming client, proxy client pool, API health checker, agent TCP monitor, and the two image-gen providers — with a dead `TimeoutConfiguration` struct in `crates/domain/agent/src/services/shared/resilience.rs` trying (and failing) to be the source of truth. Introduced `crates/shared/models/src/net.rs` with twelve named `Duration` consts (`HTTP_CONNECT_TIMEOUT`, `HTTP_DEFAULT_TIMEOUT`, `HTTP_HEALTH_CHECK_TIMEOUT`, `HTTP_AUTH_VERIFY_TIMEOUT`, `HTTP_SYNC_DEPLOY_TIMEOUT`, `HTTP_STREAM_CONNECT_TIMEOUT`, `HTTP_KEEPALIVE`, `HTTP_POOL_IDLE_TIMEOUT`, `AGENT_MONITOR_TCP_TIMEOUT`, `AGENT_READINESS_TCP_TIMEOUT`, `IMAGE_GEN_LONG_POLL_TIMEOUT`, `IMAGE_GEN_OPENAI_TIMEOUT`) so intent is explicit where values diverge (e.g. 300s for long-poll image gen, 2s for aggressive readiness probes, 15s for agent-startup grace). All 15 sites now reference these consts; every timeout preserves its previous numeric value — no runtime-behaviour change. Dead `TimeoutConfiguration` / `TimeoutType` enum deleted.

- **Consolidated further duplicate literals and removed an orphaned `AgentExtension` module.** `https://api.systemprompt.io` was inlined twice in `crates/infra/cloud/src/credentials_bootstrap.rs` (shadowing the existing `constants::api::PRODUCTION_URL`); both now reference the const. The A2A artifact-rendering extension URI `https://systemprompt.io/extensions/artifact-rendering/v1` was duplicated across 4 files — extracted to `systemprompt_models::a2a::ARTIFACT_RENDERING_URI` and wired into `agent_card.rs`, `artifact_transformer/mod.rs`, and `batch_builders.rs`. A second parallel `AgentExtension` struct in `crates/shared/models/src/a2a/agent_extension.rs` was an orphan (not in `mod.rs`, not referenced) — deleted. Production/sandbox DB hostnames (`db.systemprompt.io`, `db-sandbox.systemprompt.io`) in `swap_to_external_host` promoted to `constants::api::DB_PRODUCTION_HOST` / `DB_SANDBOX_HOST` next to the existing URL consts. `CALLBACK_TIMEOUT_SECS = 300` was declared twice (`oauth` and `checkout` modules) — lifted to a single top-level const aliased by both. User-Agent strings in the CIMD fetcher and webhook delivery service had hardcoded version suffixes (`systemprompt.io-OS/2.0`, `systemprompt.io-Webhook/1.0`) — now use `concat!("…/", env!("CARGO_PKG_VERSION"))` so the UA always matches the running binary.

- **CLI `--version` and API discovery reported stale hardcoded versions; protocol-spec versions were duplicated as literals.** `crates/entry/cli/src/args.rs:80` pinned the clap `#[command(version = "0.1.0")]` attribute to a literal, so `systemprompt --version` returned `0.1.0` regardless of the workspace version or the release tag. Swapped to `env!("CARGO_PKG_VERSION")` which clap resolves at build time from the crate's inherited workspace version. Same fix applied to the API gateway discovery endpoint (`crates/entry/api/src/services/server/discovery.rs:18`, user-visible `/` response) and the plugin marketplace generator (`crates/entry/cli/src/commands/core/plugins/generate/marketplace.rs:60`). Extracted the A2A and MCP protocol-spec versions into named constants to eliminate duplicate literals: `systemprompt_agent::A2A_PROTOCOL_VERSION = "0.3.0"` (replaces duplicates at `crates/domain/agent/src/models/web/card_input.rs:31` and `crates/entry/cli/src/commands/admin/agents/create.rs:94`) and `systemprompt_mcp::MCP_PROTOCOL_VERSION = "2024-11-05"` (replaces duplicates at `crates/domain/mcp/src/services/registry/trait_impl.rs:87` and `crates/entry/api/src/routes/agent/registry.rs:127`). These are pinned to external protocol specs — not our crate version — so the const form preserves intent while killing drift risk.

## [0.3.1] - 2026-04-22

### Fixed

- **Gateway tracing — six bugs overstating cost ~130× and hiding every downstream observability surface.** End-to-end audit (documented in `cowork-tracing.md`) of a live minimax gateway request found that cost reporting and every CLI read path (`audit --full`, `trace list`, `trace show`, `analytics conversations list`) was broken for gateway traffic. Root fixes in dependency order:

  - **`AnthropicCompatibleUpstream` honours `upstream_model`** (`crates/entry/api/src/services/gateway/upstream.rs`). Previously forwarded the raw request body unchanged, sending the client's `claude-sonnet-4-6` string to minimax regardless of `route.upstream_model`. Now computes `ctx.route.effective_upstream_model(&ctx.request.model)`, rewrites `body.model` only if it differs (pass-through stays zero-copy), and captures `response.model` into a new `UpstreamOutcome::Buffered { served_model, .. }` field so the audit layer learns what minimax actually served.

  - **`ai_requests.model` now stores the served model, not the client request** (`crates/entry/api/src/routes/gateway/messages.rs`, `crates/entry/api/src/services/gateway/audit.rs`). `GatewayRequestContext.model` is seeded from `route.effective_upstream_model()` at handler entry. `GatewayAudit::set_served_model()` overwrites `ai_requests.model` via new `AiRequestRepository::update_model` when the upstream response's `model` field differs from the route guess. Streaming path captures this from the `message_start` SSE frame via `stream_tap`.

  - **Real minimax pricing + unreachable match arm removed** (`crates/entry/api/src/services/gateway/pricing.rs`). The previous minimax branch had two identical `ModelPricing { 0.2, 1.1 }` arms (dead pattern match) at rates ~130× actual MiniMax API pricing. Replaced with per-family rates (`minimax-text-01` / `abab6.5` at $0.0002/$0.0011 per 1k, `minimax-m1` / `abab7-chat-preview` at $0.0004/$0.0022). Unknown models now fall through to `unknown()` which logs a warning and returns zero cost — missing entries are loud instead of silently overbilling. Pricing lookup moved from `GatewayAudit::new()` to `GatewayAudit::complete()` so the served model drives the rate.

  - **`ai_request_messages` populated from gateway path** (`crates/entry/api/src/services/gateway/audit.rs`, `crates/entry/api/src/services/gateway/parse.rs`). `GatewayAudit::open()` now parses the `AnthropicGatewayRequest` and inserts each message (plus any `system` prompt at `sequence_number=0`) via `AiRequestRepository::insert_message`. New `flatten_system_prompt` / `flatten_message_content` helpers join text blocks and JSON-encode tool_use / tool_result blocks. `complete()` appends the assistant response via `add_response_message`, extracted by new `parse::extract_assistant_text`. `audit <id> --full` now shows the full conversation turn instead of `"messages": []`.

  - **Gateway traces visible in `trace list` / `trace show`** (`crates/infra/logging/src/trace/list_queries.rs`). The `require_tracked` filter required `status IS NOT NULL`, which comes from `agent_tasks` — gateway requests don't create task rows, so their traces were hidden unless `--include-system` was passed. Filter dropped; `exclude_system` still drops the literal `"system"` bucket. `trace show` already renders AI summary when log events are empty, so it surfaces gateway traces as soon as they're discoverable.

  - **Gateway sessions in `analytics conversations list`** (`crates/domain/analytics/src/repository/conversations.rs`). `list_conversations` was `user_contexts`-only, populated exclusively by the agent path. Query rewritten as UNION of two CTEs: the original `agent_convs` (unchanged semantics) and a new `gateway_convs` that synthesizes rows from `ai_requests` where `task_id IS NULL`, grouped by `session_id`, counting `ai_request_messages` (populated by the Bug 3 fix). A `NOT EXISTS` guard prevents double-counting sessions that also have a `user_contexts` row.

  Added new `AiRequestRepository::update_model(id, model)` method (`crates/domain/ai/src/repository/ai_requests/mutations.rs`).

### Changed

- **Gateway helpers extracted to `gateway::flatten`** (`crates/entry/api/src/services/gateway/flatten.rs`, new). Consolidates `flatten_system_prompt`, `flatten_message_content`, `rewrite_request_model` (body JSON substitution for Anthropic-compatible upstream), and `parse_served_model` (response-body model extraction) into one module. Keeps `audit.rs` and `upstream.rs` near the 300-line coding-standards cap and isolates the JSON-at-protocol-boundary surface. Audit `build_record`, `persist_request_messages`, `persist_tool_calls` split into dedicated methods for function-length discipline.

  Verification: `cargo check --workspace` + `cargo clippy --workspace --all-targets` clean with `-D warnings`; `cargo fmt --all -- --check` clean; `systemprompt-api-tests` (429 passing) and `systemprompt-logging-tests` green. Expected end-to-end behavior: a minimax request now records cost within ±5% of the real MiniMax invoice, `audit --full` shows the full conversation, and the trace/analytics CLI commands surface gateway traffic without flags.

---

## [0.3.0] - 2026-04-22

### Added

- **LLM Gateway — `/v1/messages` inference routing.** Organisations using Claude for Work (formerly Claude Cowork) can now set `api_external_url` in their fleet MDM configuration to `https://systemprompt.io` and have every Claude Desktop inference request flow through the gateway. The gateway:
  - Exposes `POST /v1/messages` at the Anthropic wire format — fully compatible with the Claude API SDK, Claude Desktop, and any Anthropic-SDK client.
  - Authenticates with a systemprompt JWT carried in the `x-api-key` header (falls back to `Authorization: Bearer`). No additional API key is issued; the organisation's existing user JWTs serve as the credential.
  - Routes requests to any configured upstream provider based on `model_pattern` rules in the profile YAML. Supported provider types: `anthropic`, `openai` (OpenAI-compatible), `moonshot` (Kimi), `qwen`, `gemini` (stub — not yet dispatched).
  - **Anthropic upstream**: transparent byte proxy. Raw request bytes forwarded verbatim to the upstream endpoint with the upstream API key substituted; the response stream is piped back unmodified. Preserves extended thinking blocks, cache-control headers, and all Anthropic-specific SSE events exactly.
  - **OpenAI-compatible upstream**: converts Anthropic request format → OpenAI `/v1/chat/completions` format, proxies to the upstream, converts the response back to Anthropic format. For streaming, maps OpenAI SSE delta events to Anthropic `message_start` / `content_block_start` / `content_block_delta` / `message_delta` / `message_stop` SSE frames.
  - **API key resolution**: upstream API keys are resolved from the existing secrets file by secret name (`api_key_secret` in the route config). No new credential storage mechanism.
  - **Conditional mount**: the `/v1` router is only registered when `gateway.enabled: true` in the active profile — zero overhead for deployments that don't use the gateway.

- **Gateway profile configuration schema.** New `gateway` block in profile YAML (all fields optional; block absent = gateway disabled):

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
        upstream_model: "moonshot-v1-8k"   # optional: override model name sent upstream
      - model_pattern: "qwen-*"
        provider: qwen
        endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1"
        api_key_secret: "qwen_api_key"
      - model_pattern: "*"                  # fallback route
        provider: anthropic
        endpoint: "https://api.anthropic.com/v1"
        api_key_secret: "anthropic_api_key"
  ```

  Routes are evaluated in order; first `model_pattern` match wins. Patterns support `*` wildcard prefix/suffix matching. `extra_headers` map is available per route for provider-specific requirements.

- **`GatewayProvider::is_openai_compatible()`** — `const fn` on the provider enum; returns `true` for `OpenAI`, `Moonshot`, `Qwen`. Used internally to select the conversion path.

- **`GatewayRoute::find_route(model)`** — resolves the first matching route for a given model name from a `GatewayConfig`. Returns `None` if no route matches (handler returns 404).

- **`GatewayRoute::effective_upstream_model(model)`** — returns `upstream_model` if set, otherwise echoes the client-provided model name. Enables transparent model aliasing (e.g. client requests `moonshot-v1-8k`; gateway can remap to a different upstream model name without the client knowing).

- **`JwtContextExtractor::extract_for_gateway(jwt_token: &JwtToken)`** — new method on the JWT middleware extractor. Accepts a typed `JwtToken` identifier (not a raw `&str`), validates it, and returns a `RequestContext`. Used by the gateway handler to validate the `x-api-key` credential without relying on the standard `Authorization: Bearer` middleware layer.

- **`ApiPaths::GATEWAY_BASE`** constant — `/v1` path prefix for the gateway router.

- **Cowork credential-helper auth path.** Claude for Work clients configure a `Credential helper script` that prints a bearer token on stdout; core now ships the helper binary plus the matching gateway endpoints that exchange a lower-privilege credential for a short-lived JWT carrying canonical identity headers.

  Gateway endpoints (mounted under `/v1/gateway/auth/cowork/` when `gateway.enabled: true`):

  - `POST /pat` — `Authorization: Bearer <pat>` → verifies the PAT via `systemprompt_users::ApiKeyService::verify`, loads the user via `systemprompt_oauth::repository::OAuthRepository::get_authenticated_user`, returns `{token, ttl, headers}` with a fresh JWT and the canonical header map.
  - `POST /session` — stub returning `501` (dashboard-cookie exchange not yet wired).
  - `POST /mtls` — stub returning `501` (device-cert exchange not yet wired).
  - `GET /capabilities` — returns `{"modes":["pat"]}`; probes advertise which exchange modes the deployment accepts.

  The JWT-assembly + header map live in `systemprompt_oauth::services::cowork` (`issue_cowork_access`, `issue_cowork_access_with`, `CoworkAuthResult`) so the route handler in `entry/api` stays thin — it only extracts the bearer, verifies via `ApiKeyService`, and calls the oauth-domain service. Headers returned in the response body use core's canonical constants from `systemprompt_identifiers::headers::*` (`x-user-id`, `x-session-id`, `x-trace-id`, `x-client-id`, `x-tenant-id`, `x-policy-version`, `x-call-source`) so Cowork merges them into every subsequent `/v1/messages` call and the gateway middleware reads real identity on every request.

- **`systemprompt-cowork` credential helper + sync agent binary.** Standalone crate at `bin/cowork/` (excluded from the workspace so it does not compile during `cargo build --workspace` and does not land in the `systemprompt` crates.io package). Dependency footprint is deliberately minimal (`ureq` + `rustls` + `serde` + `toml` + `ed25519-dalek`) — no `tokio`, `sqlx`, or `axum`.

  - **Progressive capability ladder**: probes credential providers in descending strength (mTLS → dashboard session → PAT). First provider that returns a token wins; absent providers return `NotConfigured` and the chain falls through. No user-facing "pick a mode" step.
  - **Providers** (`src/providers/{mtls,session,pat}.rs`) share a single `AuthProvider` trait returning `Result<HelperOutput, AuthError>` where `AuthError::NotConfigured` silently advances the chain.
  - **Config**: TOML at `~/.config/systemprompt/systemprompt-cowork.toml` (or `$SP_COWORK_CONFIG`). All sections optional — absent sections mean the provider is skipped. Dev overrides: `$SP_COWORK_GATEWAY_URL`, `$SP_COWORK_PAT`, `$SP_COWORK_DEVICE_CERT`, `$SP_COWORK_USER_ASSERTION`.
  - **Cache**: signed JWT + expiry written to the OS cache dir with mode `0600` on unix. Cached token is emitted directly if valid; only on cache miss does the probe chain run.
  - **Stdout contract**: exactly one JSON object matching `{token, ttl, headers}` — Anthropic's `inferenceCredentialHelper` format. All diagnostics go to stderr. Exit 0 on success, non-zero on failure.
  - **Sync agent**: `install`, `sync`, `validate`, `uninstall` manage Cowork's `org-plugins/` mount (macOS `/Library/Application Support/Claude/org-plugins/`, Windows `C:\ProgramData\Claude\org-plugins\`, Linux `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/`) — pulling signed plugin manifests and managed MCP allowlists from the gateway.
  - **Release cadence**: tagged `cowork-v*`; `.github/workflows/cowork-release.yml` builds binaries for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, and `x86_64-unknown-linux-gnu`, attaches them to a GitHub Release with SHA256SUMS. Triggered only by the helper tag pattern; core's normal CI is untouched.
  - **Build targets**: `just build-cowork [target]` and `just build-cowork-all`.

- **`ClientId::cowork()`** constructor — returns `sp_cowork`, recognised as `ClientType::FirstParty` via the existing `sp_` prefix rule. Used by the Cowork JWT issuance path so every token issued to a Cowork session can be identified as first-party Cowork traffic in audit logs.

- **`SessionSource::Cowork`** variant + `SessionSource::from_client_id("sp_cowork") → Cowork`. Used as the `x-call-source` header value on Cowork-issued tokens so downstream middleware and analytics can distinguish Cowork sessions from Web / CLI / API / OAuth / MCP sessions.

- **`systemprompt_identifiers::PolicyVersion`** — new typed ID with `PolicyVersion::unversioned()` constructor. Exposed in the Cowork helper's header response as `x-policy-version` so a future policy-bundle-hash propagation feature plugs in without changing the wire contract.

- **`systemprompt_identifiers::headers::TENANT_ID` / `POLICY_VERSION`** — new canonical header constants (`x-tenant-id`, `x-policy-version`) alongside the existing `USER_ID`, `SESSION_ID`, `TRACE_ID`, `CLIENT_ID` family. All Cowork-issued tokens carry the full set in the response body's `headers` map.

- **Gateway provider registry — extensions can register custom upstreams.** `GatewayProvider` is no longer a closed enum; `GatewayRoute.provider` is now a free-form string tag resolved at dispatch time against a registry built at startup. Extension crates register new providers with:

  ```rust
  inventory::submit! {
      systemprompt_api::services::gateway::GatewayUpstreamRegistration {
          tag: "my-provider",
          factory: || std::sync::Arc::new(MyUpstream),
      }
  }
  ```

  The new `GatewayUpstream` trait (`async fn proxy(&self, ctx: UpstreamCtx<'_>)`) is the single integration seam. Built-in tags seeded automatically: `anthropic`, `minimax`, `openai`, `moonshot`, `qwen`. Extension-registered tags may shadow built-ins (logged as a warning).

- **MiniMax provider.** MiniMax ships an Anthropic-compatible endpoint at `https://api.minimax.io/anthropic`, so the new `minimax` tag reuses the Anthropic-compatible upstream verbatim — streaming, tool use, and `thinking` blocks pass through untouched. Example route:

  ```yaml
  gateway:
    enabled: true
    routes:
      - model_pattern: "MiniMax-*"
        provider: minimax
        endpoint: https://api.minimax.io/anthropic
        api_key_secret: minimax
  ```

  The `api_key_secret` resolves through `Secrets.custom`, so no changes to the secrets schema are required.

- **Gateway governance — full audit, policy, quota, and safety pipeline.** Every `/v1/messages` call now lands a structured audit trail, enforces tenant-scoped policy, and runs through a pluggable safety scanner before and after dispatch. This closes the product-level gap where the gateway proxied requests to MiniMax/Anthropic/OpenAI upstreams but persisted nothing beyond a one-line tracing log. For a platform whose core promise is "governance for all AI calls", this is the spine that makes the promise enforceable rather than aspirational.

  - **`ai_requests` persistence on the gateway path.** The handler mints a typed `AiRequestId` at ingress, writes a `pending` row before dispatch (with `user_id`, `tenant_id`, `session_id`, `trace_id`, `provider`, `model`, `max_tokens`, `is_streaming`), and updates it to `completed` with `input_tokens` / `output_tokens` / `cost_microdollars` / `latency_ms` once the upstream response resolves. Non-streaming responses parse the buffered JSON to extract usage + `tool_use` blocks; streaming responses run through an SSE tap (see below) that captures the same data without mutating the byte stream. On upstream error, the row flips to `failed` with `error_message` populated. Audit writes are best-effort — a DB outage logs an ERROR but never blocks the proxied request.

  - **`ai_request_payloads` table — full request/response retention.** New JSONB columns per `AiRequestId`: `request_body`, `response_body`, plus truncation flags + byte counts. 256 KB cap per side; overflow writes `NULL` for the body and a head+tail excerpt (`request_excerpt` / `response_excerpt`, 8 KB each side with a `...<truncated N bytes>...` marker). Response capture for streams reconstructs the full byte payload from the tap before persisting. Payload writes are fire-and-forget (`tokio::spawn`) so the client connection closes at upstream speed regardless of DB write latency.

  - **`ai_request_tool_calls` — `tool_use` capture + `tool_result_payload` column.** Every `tool_use` block in the response (Anthropic `content[].type == "tool_use"` for buffered JSON; `content_block_start` + `input_json_delta` accumulation for SSE) writes one row to `ai_request_tool_calls` with sequence number, `ai_tool_call_id`, `tool_name`, and `tool_input` (64 KB cap with truncation marker). New nullable `tool_result_payload JSONB` column is added to close the loop on follow-up turns — the migration is in place; the match-on-`ai_tool_call_id` upsert from the next request is plumbed for a follow-up iteration.

  - **`ai_safety_findings` table + pluggable `SafetyScanner` trait.** New async trait at `crates/entry/api/src/services/gateway/safety/` with two implementations: `HeuristicScanner` (known jailbreak prefixes → severity=medium; email regex → low; Luhn-valid 16-digit credit card → high) and `NullScanner` (for tests). Scanning runs pre-dispatch on the request and post-dispatch on the response (per-chunk SSE scanning is wired but currently reuses the final-buffered path). Findings persist with phase (`request` / `response`), severity, category, and an excerpt. Current release is warn-only — findings land in the table and can be queried, but don't short-circuit the request. The policy `safety.block_categories` field is plumbed to the dispatch path and gates a `451` short-circuit in the next iteration.

  - **`ai_quota_buckets` table + token-bucket enforcement.** Per-`(tenant_id, user_id, window_seconds, window_start)` atomic counters via `INSERT ... ON CONFLICT DO UPDATE RETURNING` — Postgres serialises contention with no application-level lock. Pre-dispatch reserves 1 request; if any configured window exceeds its hard limit, dispatch returns `429 Too Many Requests` with a `Retry-After` header and the audit row flips to `failed` with `status_code='denied_quota'`. Post-dispatch, a second update adds `input_tokens` + `output_tokens` to the same buckets. Multiple windows (e.g. 60s / 3600s / 86400s) evaluate in order; first exceeded window wins.

  - **`ai_gateway_policies` table + `PolicyResolver`.** Tenant-scoped JSONB policies composed at dispatch: `allowed_models` (list of model names — anything else returns `403 Forbidden` with audit row `status='failed'`), `max_input_tokens_per_call`, `max_tool_depth`, `quota_windows`, and `safety` (scanner list + block categories). Resolution order: tenant-specific → global (`tenant_id IS NULL`) → compiled-in `GatewayPolicySpec::permissive()` fallback. 60-second in-memory TTL cache; DB unavailability logs a warning and returns the permissive fallback rather than wedging the gateway.

  - **SSE stream tap.** `crates/entry/api/src/services/gateway/stream_tap.rs` wraps the upstream `Stream<Item = Result<Bytes, io::Error>>` and re-emits every chunk to the client byte-identical, while parsing `message_start` / `message_delta` / `content_block_start` / `content_block_delta` / `content_block_stop` frames to accumulate usage + assemble `tool_use` blocks from `input_json_delta` fragments. On end-of-stream, `tokio::spawn` fires `audit.complete(usage, tool_calls, reconstructed_body)`; on upstream error, fires `audit.fail(error)`. The tap never mutates the proxied byte stream — clients that expect byte-exact Anthropic SSE get byte-exact Anthropic SSE.

  - **`x-systemprompt-request-id` response header.** Every gateway response (success, 403 policy denial, 429 quota denial, 451 safety denial, 500 upstream error) carries the minted `AiRequestId` as `x-systemprompt-request-id: <uuid>` so Cowork and any SDK caller can grep logs or the audit table by the same key. Header is also propagated into tracing spans.

  - **Pricing table.** `crates/entry/api/src/services/gateway/pricing.rs` resolves `(provider, model) → ModelPricing { input_cost_per_1k, output_cost_per_1k }` for the Claude 4.x family (Opus / Sonnet / Haiku), MiniMax-* (flat pricing), and GPT-4o family. Unknown pairs log a `WARN` and record `cost_microdollars=0` rather than failing the request, so an operator sees the gap in logs and adds the entry without an incident. Cost computation copies the proven formula from `crates/domain/ai/src/services/core/ai_service/stream_wrapper.rs` (`(input_tokens/1000 × input_cost + output_tokens/1000 × output_cost) × 1_000_000`).

  - **New typed IDs** in `systemprompt_identifiers`: `AiSafetyFindingId`, `AiQuotaBucketId`, `AiGatewayPolicyId` — all generated (UUID-backed) with the `schema` variant for OpenAPI exposure.

  - **New domain repositories** in `systemprompt_ai`: `AiRequestPayloadRepository`, `AiSafetyFindingRepository`, `AiQuotaBucketRepository`, `AiGatewayPolicyRepository`. `AiRequestRepository::insert_with_id(id, record)` is a new public method that lets the gateway audit own ID minting at ingress (the existing `insert(record)` still exists and generates a fresh ID for internal AI-service callers).

  - **`AiRequestRecord.tenant_id: Option<TenantId>`** — new field on the write model + matching `tenant_id()` setter on `AiRequestRecordBuilder`. The underlying `ai_requests` table gained `tenant_id VARCHAR(255)` via migration `001_gateway_governance.sql` with `(tenant_id)` and `(tenant_id, created_at)` indices.

  - **`JwtContextExtractor`-driven user attribution.** The gateway handler extracts `UserId`, `SessionId`, and `TraceId` from the validated JWT context (JWT path) or from the matched `ApiKeyRecord` (API key path), and reads optional `x-tenant-id` from request headers. An `AuthedPrincipal` struct bundles these four fields into a single `GatewayRequestContext` that every downstream module (audit, quota, policy, safety) reads. Previously `JwtContextExtractor::extract_for_gateway` validated the token but its result was discarded.

  - **New dependency edge**: `systemprompt-api` now depends on `systemprompt-ai` for repository access. The gateway service module gained seven new files (`audit.rs`, `parse.rs`, `pricing.rs`, `policy.rs`, `quota.rs`, `stream_tap.rs`, `safety/{mod,heuristic,null}.rs`) and `upstream.rs` was refactored to return a typed `UpstreamOutcome` enum (`Buffered { status, content_type, body } | Streaming { status, stream }`) instead of a raw `Response<Body>`, so the service layer can intercept for audit + policy enforcement before final response assembly.

### Changed

- **Gateway dispatch rewritten around the registry.** `GatewayService::dispatch` is now a thin shim: resolve route → resolve API key → look up the registered upstream → hand off to `upstream.proxy(ctx)`. The old hard-coded `match route.provider { ... }` is gone. The `GatewayProvider` enum (and its `is_openai_compatible()` / `as_str()` methods) have been removed; `GatewayRoute.provider` is a `String`. Anthropic-passthrough and OpenAI-compatible behaviours are preserved — their bodies were moved verbatim into `AnthropicCompatibleUpstream` and `OpenAiCompatibleUpstream` in the new `upstream.rs`. Unknown provider tags now fail fast with `Gateway provider 'xxx' is not registered`.

- **Analytics: broader conversion events + UTM expansion.** `event_data` column on `analytics_events` changed to `JSONB` (was `TEXT`) to support structured payload inspection. Added `utm_content` and `utm_term` UTM parameter columns to complete the full UTM dimension set. Conversion event definitions broadened to cover a wider range of funnel actions (subscription starts, trial activations, feature adoptions).

### Included from 0.2.5

- Workspace-wide Rust-standards sweep (see [0.2.5] entry below for full detail): zero inline comments, zero `unwrap_or_default()`, annotated `serde_json::Value` protocol boundaries, regenerated SQLx offline cache.

---

## [0.2.5] - 2026-04-20

### Changed
- **Workspace-wide Rust-standards sweep.** Executed a full audit against `instructions/prompt/rust.md` and the `rust-coding-standards` skill across `crates/{shared,infra,domain,app,entry}/**/src/`. Five parallel layer agents fixed every zero-tolerance violation they found; a final pass closed the clippy-exposed stragglers. `cargo clippy --workspace --all-targets -- -D warnings` now passes clean, `cargo fmt --all -- --check` is clean, `cargo build --workspace` succeeds. Changes:
  - **Deleted** `crates/shared/models/src/validation_report.rs` — dead 9-line backward-compat re-export, not declared in `lib.rs`, zero importers (all call sites already used `systemprompt_traits::validation_report` directly).
  - **Replaced every `unwrap_or_default()` in src code** (13 occurrences across 7 files). Fixes range from propagating a `Result` (`MarkdownResponse::to_markdown()` now returns `Result<String, serde_yaml::Error>`; its `IntoResponse` impl logs + returns 500 on failure) to idiomatic combinators (`map_or_else(Vec::new, Clone::clone)` in oauth/agent repositories) to explicit `if let Ok(...)` env-var inheritance in agent subprocess spawn. The schema sanitizer's `.next().unwrap_or_default()` became a proper `if let Some(Value::Object(inner))` after an invariant check.
  - **Deleted 19 inline `//` comments** across infra/cloud (4), domain/{ai,agent,analytics,oauth} (14), and entry/cli (15). Per rust.md §3, code documents itself through naming; the only retained `//` annotations are the `// JSON: …` markers on `serde_json::Value` protocol-boundary sites (explicit exception per the `rust-coding-standards` skill).
  - **Annotated ~82 `serde_json::Value` sites in infra** as protocol/schemaless boundaries (A2A JSON-RPC, MCP schemas, webhook payloads, dynamic DB admin queries, log visitors, JSON-Schema trees). Triage reports for all five layers written to `reports/audit/{shared,infra,domain,app,entry}-json-triage.md` (gitignored) with counts of Keep+annotate / Refactor / Defer categories; ~24 refactorable sites and ~106 deferred (API-surface) sites enumerated there for follow-up PRs.
- **Regenerated workspace `.sqlx` offline cache.** Commit `a55b1570e` (analytics conversion + utm) added `utm_content`, `utm_term`, and `event_data` columns to the live DB but the workspace-level sqlx query cache was not regenerated, so `cargo check -p systemprompt-analytics` failed with `SQLX_OFFLINE=true`. Cache now reflects current schema; analytics crate compiles clean again.

### Fixed
- `MarkdownResponse::to_markdown()` signature changed from `fn(&self) -> String` to `fn(&self) -> Result<String, serde_yaml::Error>`. The previous version silently swallowed frontmatter serialization failures via `unwrap_or_default()` and produced a response with no frontmatter. Callers now see the error or (at the HTTP boundary) a logged 500. Breaking for any external consumer of `MarkdownResponse::to_markdown()`; there are none in this repository.

### Audit
- Post-sweep verification greps confirm **zero** occurrences of `.unwrap()`, `unwrap_or_default()`, `panic!`, `todo!`, `unimplemented!`, `unsafe`, `///` doc comments, and `TODO|FIXME|HACK` in any non-test `src/` file across the workspace. `println!`/`eprintln!` retained only at legitimate CLI-output boundaries and in the `config/schema_validation` build-script helper (already guarded with `#[allow(clippy::print_stderr, clippy::print_stdout)]`).

## [0.2.4] - 2026-04-20

### Fixed
- **`admin agents registry` now defaults to the active profile's `api_external_url`.** Previously the command hard-coded `http://localhost:8080` as its gateway URL, so `systemprompt admin agents registry` failed with `Connection refused` on any profile that used a non-default port (e.g. `just setup-local ... 8081 5434`). The hint string on `--url` still advertised "default: http://localhost:8080" even after a user pointed a profile at a different host. Fix: read the active `ProfileBootstrap::get().server.api_external_url` first; fall back to `http://localhost:8080` only if no profile is loaded. `--url` still overrides both.

## [0.2.3] - 2026-04-20

### Fixed
- **Drop cloud-auth requirement for local-trial CLI sessions.** On a fresh template clone with `just setup-local`, the CLI gated a wide set of local-capable operations (`admin agents tools`, `plugins mcp tools/call`, `core contexts list`, trace lookups) behind `Cloud authentication required. Run 'systemprompt cloud auth login' to authenticate.`. Root cause: `SessionKey::from_tenant_id(Some("local_dev"))` returns `SessionKey::Tenant(...)`, not `SessionKey::Local`, so the `session_key.is_local()` branch in `create_new_session` was skipped and `CredentialsBootstrap::require()` fired. `resolve_local_user_email` had the same behavior inside the local-session branch when `session_email_hint` was absent. Fix: centralise the "is this a local-trial profile?" rule on `CloudConfig::is_local_trial()` / `Profile::is_local_trial()` (no `cloud` block, `tenant_id` starts with `local_`, or `validation ∈ {Warn, Skip}`); `create_new_session` now also treats local-trial profiles as local; `resolve_local_user_email` falls back to `admin@localhost.dev` — matching the address `demo/00-preflight.sh` uses, so CLI- and demo-created admin sessions share a user row. Genuine cloud entrypoints (`cloud sync`, `cloud tenant select`, `admin session login`, `admin session switch`) are unchanged and still require cloud credentials. `bootstrap.rs`' duplicated 12-line local-profile predicate now delegates to the shared helper.

## [0.2.2] - 2026-04-17

### Fixed
- **macOS build fix — `statvfs` type mismatch in health endpoint.** `get_disk_usage()` in `systemprompt-api` failed to compile on macOS (Darwin) because `nix::sys::statvfs` returns `u32` for `blocks()`, `blocks_available()`, and `blocks_free()` on macOS but `u64` on Linux, while `fragment_size()` returns platform-varying types. The `saturating_mul` calls required matching types. Fix: explicit `u64::from()` casts on all `statvfs` field accesses so the arithmetic is platform-independent.

### Changed
- Docs sweep: refreshed READMEs across all 30 crates to align with the 0.2.x naming and current feature matrix.
- Relocated generator asset/build/markdown/sitemap unit tests out of `crates/app/generator/tests/` into the dedicated test workspace at `crates/tests/unit/app/generator/src/` to match the "test crates live outside the main workspace" rule. Added missing `unit_tests` module to the scheduler test workspace.

## [0.2.1] - 2026-04-16

### Fixed
- **Idempotent agent migrations — fix startup crash on existing databases.** Migrations `003_a2a_v1_task_states.sql` and `004_ai_requests_task_fk.sql` could brick service startup on sites with pre-existing data. Root cause: `SqlExecutor::execute_statements_parsed` splits SQL on semicolons and runs each statement as a separate `execute_raw` call against the connection pool, so the `BEGIN`/`COMMIT` wrapper in migration 003 was a no-op (each statement auto-committed on potentially different connections). If any statement succeeded but the migration recording failed, the next startup retried the migration and hit already-applied DDL. Three fixes: (1) removed the ineffective `BEGIN`/`COMMIT` from migration 003, (2) added missing `UPDATE` for `'pending'` → `'TASK_STATE_PENDING'` status value that would cause the CHECK constraint to reject existing rows, (3) wrapped the `ADD CONSTRAINT` in migration 004 with an `IF NOT EXISTS` guard via a `DO` block so re-running the migration after a partial failure is safe.
- **Gemini schema sanitizer — nullable & $ref handling.** `ProviderCapabilities::gemini()` now reports `features.references = false` and `features.definitions = false`, so the sanitizer strips `$ref` / `$defs` / `definitions` before the request reaches Gemini. Gemini's `FunctionDeclaration.parameters` uses `google.api.JsonSchema`, which rejects those keywords with `400 INVALID_ARGUMENT`.
- **Nullable normalisation in `SchemaSanitizer`.** New `normalize_nullable` pre-pass rewrites both JSON-Schema nullable forms into Gemini/OpenAPI `nullable: true`: `{"type": ["string", "null"]}` collapses to `{"type": "string", "nullable": true}`, and `{"anyOf": [{"type": "X"}, {"type": "null"}]}` collapses to `{"type": "X", "nullable": true}`. Non-null `anyOf` unions and `type` arrays without a `"null"` sibling are left untouched. Runs before composition stripping so the result survives the rest of the pipeline.
- **Analytics — per-agent cost breakdown reconciles with totals.** `CostAnalyticsRepository::get_breakdown_by_agent` now returns an always-present `'unattributed'` aggregate row alongside the top-N attributed agents, via a `UNION ALL` of (INNER JOIN'd attributed spend) + (unattributed spend with `task_id IS NULL OR agent_name IS NULL`). The invariant `sum(breakdown_by_agent.cost) == get_summary().total_cost` now holds for every window. An in-flight edit had switched to a plain `INNER JOIN`, silently dropping ad-hoc / context-less AI spend from the governance audit — exactly the shadow-AI blindspot the report exists to surface. `LIMIT` only bounds the attributed top-N; the unattributed row is never truncated. Four new reconciliation tests in `crates/tests/unit/domain/analytics/src/repository/costs.rs` lock the invariant in place (all-attributed, mixed-null, limit-survival, empty-window).
- **Agent extension — registered unreleased `003_a2a_v1_task_states.sql` migration.** Found during this release: `crates/domain/agent/schema/migrations/003_a2a_v1_task_states.sql` was added during the 0.1.22 A2A v1 protocol upgrade but never registered in `AgentExtension::migrations()`, so the live UPDATE that rewrites legacy `submitted`/`working`/... rows to `TASK_STATE_*` SCREAMING_SNAKE_CASE and tightens the CHECK constraint had never run on any deployed instance. Any database with pre-0.1.22 task rows would have been in an inconsistent state. Migration is now wired up and runs on next migration sweep.

### Schema
- **`ai_requests.task_id` is now a proper FK to `agent_tasks(task_id)`.** New migration `crates/domain/agent/schema/migrations/004_ai_requests_task_fk.sql` normalises the column type from `VARCHAR(255)` to `TEXT` (matches parent PK), nulls out pre-existing orphaned references (preserving cost/token data), and installs `FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id) ON DELETE SET NULL`. From here on, orphaned `task_id` values are structurally impossible, and deleting an agent task rolls its historical AI spend up under `'unattributed'` in the cost breakdown rather than cascading away audit data. `systemprompt-agent` now declares `"ai"` as an explicit extension dependency so the migration runs after the `ai_requests` table exists. Migration placement rationale: ai (weight 35) loads before agent (40), so a cross-domain FK from `ai_requests → agent_tasks` must be installed from the agent side.

### Removed — Dead `CreateAiRequest` insert path
- Deleted `CreateAiRequest` struct and `AiRequestRepository::create()` method from `crates/domain/ai/src/repository/ai_requests/`, plus associated re-exports in `crates/domain/ai/src/lib.rs`, `repository/mod.rs`, and `ai_requests/mod.rs`. The struct had no `task_id` field and no production callers; its existence invited a future bug where a new caller would use it and produce unattributable AI spend rows. The live insert path remains `AiRequestRecord` + `AiRequestRepository::insert()`, which already carries `task_id: Option<TaskId>`. BREAKING for any external crate importing `CreateAiRequest`; there are none in this repository.

### Chores
- Workspace bumped to 0.2.1; per-crate descriptions swept (b5b13d59c).
- **Cargo feature-flag sweep.** Removed unused / always-on feature gates across the workspace: `systemprompt-extension` (`web`, `plugin-discovery`), `systemprompt-logging` (empty `web`), `systemprompt-database` (`api` + dead optional `axum`), `systemprompt-mcp` (empty `cli`), `systemprompt-oauth` (`web`), `systemprompt-agent` (`web`, empty `cli`), `systemprompt-analytics` (`web`), `systemprompt-scheduler` (empty block), `systemprompt-cloud` (empty `test-utils`). Inlined previously-optional deps (axum, tower, tower-http, bytes, jsonwebtoken, tokio-stream, urlencoding) and stripped ~40 `#[cfg(feature = ...)]` gates. Legitimate gates kept: `models/web`, `traits/web`, `identifiers/sqlx`, `template-provider/tokio`, `logging/cli`, `runtime/geolocation`, `analytics/geolocation`, `generator/image-processing`, and the facade crate's user-facing feature matrix.

### Services Config Migration (Phases 1-4)

A workspace-wide breaking change to the services configuration layer.

- **Phase 1 — Schema**: `ServicesConfig` grew first-class `skills` and `content` fields; `PluginConfig` gained `content_sources` bindings; both `ServicesConfig` and `PartialServicesConfig` are locked with `#[serde(deny_unknown_fields)]`; `ServicesConfig::validate()` now enforces plugin bindings and skill map-key integrity.
- **Phase 2 — WebConfig**: deleted the 3-field stub `WebConfig` in `systemprompt-models` and switched `ServicesConfig.web` to `Option<systemprompt_provider_contracts::WebConfig>` so the rich branding/colors/typography/layout config round-trips through the loader. Breaking for any caller constructing the stub directly.
- **Phase 3 — Loader**: `ConfigLoader` is now the single loader with recursive `includes:` resolution and cycle detection. Removed `EnhancedConfigLoader`, `IncludeResolver`, `ConfigLoader::discover_and_load_agents`, and `ConfigWriter::add_include`. Loading is now pure — no auto-discovery side effects on `config.yaml`. Users must list every include explicitly.
- **Phase 4 — Callers**: `cloud profile show` and all remaining call sites migrated to `ConfigLoader::load()`.

### Phase 5 — Typed-ID migration (trait surfaces + DTOs)

- Migrated `ContextProvider`, `UserProvider`, `RoleProvider` trait surfaces from raw `&str` to typed identifiers (`UserId`, `ContextId`, `SessionId`). Breaking for any external impl.
- Waves 1–5 (commits 13568bcfa…806cc2844) covered canonical models, A2A protocol, oauth/webauthn, AI rows, tracing, app sync/generator, and CLI residuals.
- DTO sweep: migrated the remaining raw `String` ID fields across cloud DTOs, services models, AI rows, analytics events, A2A protocol messages, and API/CLI surfaces to `systemprompt_identifiers` typed IDs. Serialization is unchanged (typed IDs round-trip as plain strings).
- Wave 7 — **completed**: all 69 remaining raw `String` ID fields across shared traits, shared models, infra (security claims), domain (users, analytics, ai, oauth, agent), app/sync, entry/api webauthn+anonymous+proxy, and entry/cli plugins/content/logs migrated to typed identifiers. `LogId` gained `JsonSchema` support. `WebAuthnService::finish_registration_with_token` and `WebAuthnService::finish_registration` now return `UserId` instead of `String`. Vendor/external IDs (WebAuthn FIDO2 credentials, A2A third-party agent-card skill IDs, third-party webhook endpoint IDs, external LLM model names, CTA button action identifiers) kept as `String` with `// JSON:` justification comments per the narrow exception in CLAUDE.md. Clap CLI arguments that accept user-provided partial lookups (`ShowArgs.id`, `AuditArgs.id`, etc.) annotated with `// CLI:` and kept as `String` by design.

### Removed — Dead authorization stubs

- Deleted `crates/domain/oauth/src/services/auth_provider.rs` in its entirety. `JwtAuthProvider`, `JwtAuthorizationProvider`, and `TraitBasedAuthService` were dead since v0.0.1: zero production callers, and `JwtAuthorizationProvider::{authorize, get_permissions}` silently returned `Ok(true)` / `Ok(vec![])` regardless of input — a latent authorization footgun. Real permission logic continues to live in `JwtClaims::get_permissions()` and `crates/domain/mcp/src/middleware/rbac.rs`.
- Collapsed the `AuthorizationProvider` trait and `AuthProvider` trait entirely — both were single-impl traits with no call sites. Removed associated dead types: `AuthAction`, `AuthPermission`, `TokenPair`, `TokenClaims`, `DynAuthProvider`, `DynAuthorizationProvider`. BREAKING for any external crate importing these names; there are none in this repository.
- Removed `JwtAuthProvider::{refresh_token, revoke_token}` which returned `"not yet implemented"` errors and had zero callers. The real OAuth refresh/revoke lifecycle uses `OAuthRepository` and the token endpoints — unaffected.

### Fixed

- Zero-warning, zero-error build across workspace (`cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all --check`).
- Resolved clippy `needless_borrow` in `crates/entry/api/src/routes/oauth/endpoints/anonymous.rs` and `.../token/generation.rs`.
- Resolved clippy `useless_conversion` and `single_match_else` in `crates/entry/cli/src/commands/admin/agents/message.rs` and `.../cloud/sync/admin_user/sync.rs`.
- Dropped unused parameters in `AgentOrchestrationDatabase::{mark_failed, get_unresponsive_agents}`, `MonitorService::cleanup_unresponsive_agents`, and `a2a_server::handlers::request::validation::should_require_oauth` — signatures no longer lie about what the implementation uses.
- Removed 15 forbidden doc comments from `crates/shared/models/src/macros.rs` (standards: no `///` in production code).
- Removed 1 unnecessary path qualification in `crates/domain/agent/src/services/a2a_server/auth/validation.rs`.

## [0.1.22] - 2026-04-07

### Changed
- **A2A Protocol v1.0.0 Migration** — upgrade from v0.3.0 to the first stable release (Linux Foundation, March 2026)
  - TaskState: kebab-case to `TASK_STATE_*` SCREAMING_SNAKE_CASE (`"submitted"` -> `"TASK_STATE_SUBMITTED"`)
  - MessageRole: `"user"`/`"agent"` to `"ROLE_USER"`/`"ROLE_AGENT"`, now a typed enum
  - Part: tagged enum (`kind` discriminator) to untagged (field-presence discrimination)
  - FileWithBytes renamed to FileContent; `bytes` now optional, added `url` field for URL-referenced files
  - Message: removed `kind` field, `id` renamed to `message_id`
  - Task: removed `kind` field, added `created_at`/`last_modified` timestamps
  - Artifact: `name` renamed to `title`
  - AgentCard: collapsed `url`/`preferred_transport`/`additional_interfaces` into `supported_interfaces` array with per-interface protocol version
  - TransportProtocol renamed to ProtocolBinding (type alias kept)
  - JSON-RPC methods: PascalCase (`"message/send"` -> `"SendMessage"`, `"tasks/get"` -> `"GetTask"`, etc.)

### Fixed
- Resolve all build warnings and clippy errors across workspace
  - Add missing `Debug` derives on `BuildMetadataParams`, `HtmlBuilder`, `TokenGenerationParams`, `AuthCodeValidationParams`
  - Fix ambiguous glob re-export of `validation` module in OAuth endpoints
  - Allow `struct_field_names` on A2A `Message` (protocol-required field name)
  - Replace redundant closures with function references in agent URL extraction
  - Add `const fn` to `TaskState::is_terminal()`, `can_transition_to()`, and `role_to_str()`
  - Use `Self` instead of concrete type in `TaskState::can_transition_to()` parameter

### Added
- Database migration `003_a2a_v1_task_states.sql` for task status value migration
- TaskState `is_terminal()` and `can_transition_to()` methods for state machine validation
- Backward-compatible task state parsing (accepts both old and new format strings)

## [0.1.21] - 2026-04-01

### Fixed
- Remove silent error swallowing in `DatabaseLayer::flush()` — all DB log write failures are now reported with entry count
- Logging initialization order: `init_logging(db_pool)` now works regardless of whether `init_console_logging()` was called first

### Changed
- Replace `DatabaseLayer` with `ProxyDatabaseLayer` architecture — subscriber is always initialized with a proxy that accepts a DB pool attachment at any time
- Move `AppContext` construction logic from `new_internal()` into `AppContextBuilder::build()` — builder owns its construction
- Move `init_logging()` call earlier in `AppContextBuilder::build()` — immediately after DB pool creation, before extension discovery
- Extract `AppContextBuilder` into `crates/app/runtime/src/builder.rs`
- Extract `ProxyDatabaseLayer` and shared span/event helpers into `crates/infra/logging/src/layer/proxy.rs`
- Remove redundant `init_logging()` call from `serve.rs`

## [0.1.20] - 2026-04-01

### Changed
- Upgrade `rmcp`/`rmcp-macros` from 1.1 to 1.3
- Simplify MCP `StreamableHttpServerConfig` to use library defaults instead of manual field construction
- Adapt MCP HTTP client to rmcp 1.3 API: replace removed `AuthRequiredError` with `UnexpectedServerResponse`
- Rebrand README messaging: reposition from "production infrastructure for AI agents" to "AI governance layer" with compliance-first positioning (SOC 2, ISO 27001, HIPAA, FedRAMP)
- Update README navigation: "Playbooks" → "Skills"

### Added
- `ensure_project_scaffolding()` function in cloud init — auto-creates `services/` and `web/` directories during local tenant setup
- Project scaffolding step integrated into local tenant creation workflow (runs before profile setup)

### Refactored
- Resolve all remaining clippy errors and warnings to achieve zero-warning build
- Introduce parameter structs for `too_many_arguments` in agent services (Wave 2)
- Eliminate all redundant closure violations (Wave 1)
- Split large files: complete `deploy/mod.rs` split and file split extractions from source files
- Remove `unsafe` blocks and convert static SQL to compile-time verified macros

### Removed
- Clean up ~120 stale SQLx query cache files from sync crate

## [0.1.19] - 2026-03-31

### Added
- `CloudEnterpriseLicenseInfo` struct for domain-based enterprise licensing
- `enterprise` field on `UserMeResponse` (optional, backward-compatible)
- `EnterpriseLicenseInfo` type alias
- Structured streaming with `StreamChunk` enum for typed AI provider responses with token usage tracking
- Pricing-based cost calculation for streaming responses
- Authenticated `/api/v1/health/detail` endpoint with full system diagnostics (split from public health check)
- Email validation module (`validation.rs`) with shared `is_valid_email` helper
- ConnectInfo fallback for IP extraction in bot detector and IP ban middleware
- `geolocation` feature flag for optional GeoIP/MaxMind dependency in analytics and runtime

### Changed
- Simplify public `/health` endpoint to a lightweight DB-only check (fast for load balancers)
- Replace `tokio::process::Command("df")` disk usage with synchronous `libc::statvfs` syscall
- Make `CliService` conditionally compiled behind `cli` feature flag in logging crate
- Reduce default tokio features in workspace (remove `fs`, `process`, `signal` from default set)
- Replace blocking `std::sync::Mutex` with `tokio::sync::Mutex` in Gemini AI provider to prevent tokio worker thread stalls
- Agent sub-processes now start with a clean environment (`env_clear`) instead of inheriting all parent secrets
- Filter system traces and unknown status from trace list by default

### Security
- Fix OAuth redirect URI bypass: full URLs can no longer match relative URI registrations
- Fix WebAuthn user ID spoofing: completion handler now verifies authenticated user identity via auth token instead of trusting query parameter
- Remove wildcard CORS headers from WebAuthn completion endpoint
- Enforce 120-second expiry on WebAuthn registration and authentication challenges
- Add Shannon entropy validation for PKCE code challenges
- Block internal/private IP addresses in OAuth resource URI validation
- Use constant-time comparison (`subtle` crate) for sync token authentication
- Block symlinks and hardlinks in tarball extraction with canonical path validation
- Unify authorization code error messages to prevent enumeration attacks

### Refactored
- **CLI architecture remediation**: eliminate all `unwrap_or_default()`, `unsafe`, unlogged `.ok()`, and `println!()` violations across 8 CLI domains (admin, analytics, cloud, core, infrastructure, plugins, web, build)
- Split 14 oversized CLI files (>300 lines) into focused submodules — zero files now exceed the 300-line limit
- Extract magic numbers to named constants across analytics and infrastructure commands
- Refactor long functions (>75 lines) in analytics agents/show, sessions/live, and tools/show
- Replace `unsafe { std::env::set_var() }` in cloud profile/sync with safe `ProfileBootstrap::init_from_path()` config propagation
- Replace raw `std::env::var()` calls in cloud commands with Config-based alternatives
- **Struct consolidation**: rename duplicate `ToolModelConfig` (all-optional) to `ToolModelOverride`, resolve `Settings` collision into `ServicesSettings`/`DeploymentSettings`, deduplicate `RenderingHints` (CLI now imports from models crate)
- Convert `ToolContext` ID fields from raw `String` to typed identifiers (`SessionId`, `TraceId`, `AiToolCallId`)
- Convert image generation model ID fields from raw `String` to typed identifiers (`UserId`, `SessionId`, `TraceId`, `McpExecutionId`)
- **Eliminate inline SQL from CLI**: move 10 inline queries from `logs/show.rs`, `logs/export.rs`, and `logs/summary.rs` to `TraceQueryService` with dedicated query modules (`log_lookup_queries.rs`, `log_summary_queries.rs`)
- **Typed IDs for trace models**: replace 6 remaining `String` ID fields with typed identifiers (`LogId`, `AiRequestId`, `ExecutionStepId`) across `LogSearchItem`, `AiRequestListItem`, `AiRequestDetail`, `AuditLookupResult`, `ExecutionStep`, `AiRequestInfo`
- **DRY identifier definitions**: consolidate hand-written identifier structs into `define_id!()` macro invocations, removing ~2,500 lines of duplicated boilerplate across 14 identifier modules
- Consolidate shared utilities and per-crate `.sqlx/` caches for publish workflow
- Config cleanup: encapsulate visibility, remove dead code across config and logging crates
- **Code quality sweep across all layers** (139 files): remove clippy suppressions, fix forbidden constructs, eliminate silent error patterns
  - Remove `#[allow(clippy::*)]` suppressions by fixing underlying issues: `cognitive_complexity` (split functions), `too_many_arguments` (parameter structs), `struct_excessive_bools` (bitflags/enums), `print_stdout` (CliService::output/std::io::Write), `expect_used` (proper error propagation), `unnecessary_wraps`, `struct_field_names`, `empty_structs_with_brackets`, `option_option` (CategoryIdUpdate enum), `enum_variant_names`
  - Replace `CommandDescriptor` 6-bool struct with u8 bitflags pattern and const accessor methods
  - Introduce parameter structs: `TenantSessionParams`, `NonStreamingRequest`, `SessionStoreParams`, `ToolCallParams`, `TrackingParams`, `ReconcileSuccessParams`, `BuildContextParams`, `AuthCodeValidationParams`
  - Remove anyhow bridges in `AiError` and `AgentError`: replace `DatabaseError(#[from] anyhow::Error)` with `DatabaseError(String)`
  - Replace `println!`/`eprintln!` with `std::io::Write` across infra/logging CLI display and startup validation
  - Fix all `unwrap_or_default()` in CLI and domain code with explicit error handling
  - Fix silent error patterns: convert `let _ =` and `.ok()` to `tracing::warn!` or proper propagation across agent, mcp, scheduler, runtime, and API layers
  - Replace `Vec<EndpointRateLimit>` for rate limit config (eliminate struct_field_names)
  - Split `ProviderCapabilities` into `SchemaComposition` + `SchemaFeatures` sub-structs
  - Replace `process::exit()` with proper error propagation in CLI bootstrap
- Extract trace/logging queries into dedicated modules (`log_search_queries.rs`, `request_queries.rs`, `tool_queries.rs`, etc.)
- Remove dead `show_helpers.rs` and unused agent lib.rs clippy allow-list
- **Module visibility hardening**: convert `pub mod` to `pub(crate) mod` for internal modules across 7 domain crates (agent, ai, analytics, users, oauth, content, mcp) — reduces public API surface while preserving re-exports
- **Rename `models::ContentError` to `ContentValidationError`**: resolve naming collision with the operational `error::ContentError` in the content crate
- Fix `McpCspDomains` field references (`connect_domains` -> `connect`, `resource_domains` -> `resources`, etc.)
- Fix `BuildContextParams` call sites to use struct construction instead of positional args
- **Coding standards compliance sweep**:
  - Delete 20 dead `.rs` files and 4 dead `.sql` files not declared in any `mod.rs`
  - Convert 7 static `sqlx::query()` calls to compile-time verified `sqlx::query!()` / `sqlx::query_scalar!()` macros
  - Remove `unsafe` block in config manager: replace `std::env::set_var` with in-process `HashMap` for secret resolution
  - Remove `unsafe` block in health check: replace `libc::statvfs` FFI with `nix::sys::statvfs` safe wrapper
  - Split 6 files exceeding 400 lines into focused submodules: `audit_queries.rs`, `ai_trace_display.rs`, `secrets_bootstrap.rs`, `file_bundler.rs`, `deploy_steps.rs`, `profile_steps.rs`
- Fix `Arc<AnalyticsService>` to `Arc<dyn AnalyticsProvider>` coercion in session middleware
- Fix `CloudPaths` API consumers after `get_cloud_paths()` return type change
- Remove unused `_pool` parameter from `CleanupRepository::new_with_write_pool`

### Fixed
- Sub-process binary resolution now checks both `target/release` and `target/debug`, preferring the newest by mtime — matches justfile behavior so MCP servers and agents find the correct binary during development
- MCP binary validation uses dynamic bin directory resolution instead of hardcoding `target/release`
- Fix test compilation across `systemprompt-generator` and `systemprompt-sync`
- Remove needless `..Default::default()` in API JWT config
- Fix `bool as Option<bool>` invalid cast in trace list queries
- Populate AI trace summary fields (`total_cost`, `total_tokens`, `total_latency`) that were previously always zero

## [0.1.18] - 2026-03-05

### Changed
- Upgrade Rust edition from 2021 to 2024
- Reorder imports across all crates to comply with Rust 2024 edition formatting rules
- Change `unsafe_code` workspace lint from `forbid` to `deny`
- Parallelize prerender pipeline: concurrent source processing, item rendering, and content enrichment
- Replace regex-based TOC heading ID injection with string search (removes `regex` dependency from generator)

### Removed
- Remove TUI OAuth client seed data and configuration
- Remove TUI testing plan
