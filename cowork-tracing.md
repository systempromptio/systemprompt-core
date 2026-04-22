# Cowork Gateway — Tracing Bug Report

**Date:** 2026-04-22  
**Observed via:** `systemprompt infra logs` CLI, source audit  
**Trigger:** End-to-end trace of a real "hello" conversation routed through minimax gateway

---

## Context

Three gateway requests were recorded at 14:32 on 2026-04-22 (local tenant `local_19c2d6bdf6c`):

| request_id | tokens (in/out) | recorded cost | actual platform cost | trace_id |
|---|---|---|---|---|
| `762ff78b` | 74 / 117 | $0.14 | ~$0.001 | `8b427751` |
| `ab6ed5a7` | 33329 / 58 | $6.73 | ~$0.03 | `1ab415dd` |
| `78b9fa35` | 5478 / 28 | $1.13 | ~$0.005 | `726c2cdb` |

All three show `provider: minimax`, `model: claude-sonnet-4-6`. Total recorded cost: **$7.99**. Actual minimax billing: **≈$0.06** (confirmed against platform balance). Factor of ~130× overstatement.

---

## Bug 1 — Cost calculation: wrong pricing rates for minimax

### What happens

`pricing.rs::lookup()` applies `input_cost_per_1k: 0.2, output_cost_per_1k: 1.1` to every minimax request regardless of model:

```rust
// crates/entry/api/src/services/gateway/pricing.rs
if provider.eq_ignore_ascii_case("minimax") {
    return match m {
        x if x.contains("minimax-m") => ModelPricing { input_cost_per_1k: 0.2, output_cost_per_1k: 1.1 },
        _ => ModelPricing { input_cost_per_1k: 0.2, output_cost_per_1k: 1.1 },  // identical fallback
    };
}
```

At those rates for `ab6ed5a7`: `(33329/1000 × 0.2) + (58/1000 × 1.1) = $6.67 + $0.06 = $6.73`.

### What's wrong

1. The rates are placeholder/wrong. MiniMax API pricing is orders of magnitude cheaper (confirmed: actual spend ~$0.06 for all three requests combined).
2. The model being priced is `claude-sonnet-4-6` — an Anthropic model name — because the **requested** model is stored, not the **upstream** model actually dispatched to minimax (see Bug 2). So the minimax pricing branch never even matches `"minimax-m"`, it always hits the `_` arm.
3. The two `minimax-m` and `_` arms are identical — the pattern match is dead code.

### Files
- `crates/entry/api/src/services/gateway/pricing.rs` — rates need correcting once upstream model is captured (Bug 2 must be fixed first)

---

## Bug 2 — Model identity: requested model stored, not upstream model served

### What happens

In `routes/gateway/messages.rs`, the gateway context is built from the client request:

```rust
// crates/entry/api/src/routes/gateway/messages.rs
let gateway_ctx = GatewayRequestContext {
    ...
    provider: route.provider.clone(),       // "minimax"
    model: gateway_request.model.clone(),   // "claude-sonnet-4-6" ← client's request
    ...
};
```

`AnthropicCompatibleUpstream::proxy()` then forwards the raw body unchanged to minimax's endpoint:

```rust
// crates/entry/api/src/services/gateway/upstream.rs
let mut req = client
    .post(&url)
    .header("x-api-key", ctx.api_key)
    ...
    .body(ctx.raw_body);  // raw body includes model: "claude-sonnet-4-6"
```

What minimax actually serves back — and the `model` field in the upstream response — is never read or stored.

### What's wrong

1. The DB records `model = "claude-sonnet-4-6"` for a request that never touched Anthropic.
2. `GatewayRoute::effective_upstream_model()` exists and is used by `OpenAiCompatibleUpstream`, but `AnthropicCompatibleUpstream` ignores it entirely. A route config with `upstream_model: "MiniMax-Text-01"` has no effect for Anthropic-compatible upstreams.
3. The actual model minimax dispatched is in the JSON response (`response.model`), which is saved to `ai_request_payloads.response_body` — but never surfaced or stored back to `ai_requests.model`.
4. Pricing for minimax is applied against an Anthropic model string, so the correct pricing branch (`minimax-m` prefix check) can never match.

### Impact chain
Bug 2 → Bug 1 (wrong model name → wrong pricing branch → wrong cost).

### Files
- `crates/entry/api/src/services/gateway/upstream.rs` — `AnthropicCompatibleUpstream::proxy()` must rewrite `model` in the body using `effective_upstream_model`, and parse the response `model` field back
- `crates/entry/api/src/routes/gateway/messages.rs` — `gateway_ctx.model` should be set to the effective upstream model, or updated post-response
- `crates/entry/api/src/services/gateway/pricing.rs` — once model is correct, update minimax pricing rates to match actual MiniMax API pricing

---

## Bug 3 — Messages array always empty in `audit --full`

### What happens

```json
{
  "messages": [],
  "tool_calls": []
}
```

### Root cause

The CLI `audit` command fetches messages from the `ai_request_messages` table:

```sql
-- crates/infra/logging/src/trace/audit_queries.rs
SELECT role, content, sequence_number
FROM ai_request_messages WHERE request_id = $1
```

But the gateway audit (`GatewayAudit`) **never writes to `ai_request_messages`**. It writes the raw request body to `ai_request_payloads.request_body`:

```rust
// crates/entry/api/src/services/gateway/audit.rs
self.payloads.upsert_request(&self.ctx.ai_request_id, UpsertPayloadParams { body: body_json, ... }).await
```

The request body contains the full `messages: [...]` array as JSON, but no code parses it and inserts the individual messages into `ai_request_messages`. The `ai_request_payloads` table is never queried by the audit command — it's a separate, unread table.

### Files
- `crates/entry/api/src/services/gateway/audit.rs` — `open()` must parse `messages` from the request body and insert rows into `ai_request_messages`
- `crates/infra/logging/src/trace/audit_queries.rs` — alternatively, the audit query should also join/union against `ai_request_payloads.request_body` as a fallback

---

## Bug 4 — Trace events never written; `trace show` always empty

### What happens

```
systemprompt infra logs trace show 8b427751 --all
⚠ No events found for trace: 8b427751
```

### Root cause

The gateway handler generates a `trace_id` (either from JWT `execution.trace_id` or a freshly generated `TraceId::generate()` for API key auth) and stores it in `ai_requests.trace_id`. But no trace span events are ever emitted to the trace event table during gateway request processing.

The `trace show` command queries a trace events table (via `ai_trace_queries.rs`). Those events are populated by the agent execution path (task runner, tool calls, etc.) — not by the gateway path. Gateway requests have a `trace_id` FK but no corresponding event rows.

### Impact
- `systemprompt infra logs trace list --since 1h` always returns "No traces found" even when gateway requests occurred
- `systemprompt infra logs trace show <id>` always empty for gateway-originated traces
- No way to correlate a gateway request to its execution context via the CLI

### Files
- `crates/entry/api/src/services/gateway/service.rs` — `GatewayService::dispatch()` should emit trace events: request_start, upstream_call, completion (or failure)
- `crates/entry/api/src/routes/gateway/messages.rs` — trace span should be opened at handler entry, closed at response

---

## Bug 5 — `analytics conversations list` returns empty

### What happens

```
systemprompt analytics conversations list
⚠ No conversations found
```

Even though 3 gateway requests completed successfully at 14:32.

### Root cause (suspected)

The conversations analytics almost certainly queries a `conversations` table (or an aggregate keyed on `session_id`). Gateway requests store `session_id` from the JWT/API-key auth context, but there is no code that creates or updates a `conversations` record when a gateway request lands. The conversations table is likely only populated via the agent/task execution path, not the raw gateway path.

### Investigation needed
- Identify the conversations table schema and its write path
- Check whether `session_id` on `ai_requests` is sufficient to infer a conversation, or whether a `conversations` upsert is needed in `GatewayService::dispatch()`

---

## Bug 6 — `AnthropicCompatibleUpstream` silently ignores `upstream_model`

### What happens

`GatewayRoute.upstream_model` is documented as the override sent to the upstream provider. `OpenAiCompatibleUpstream` respects it:

```rust
let upstream_model = ctx.route.effective_upstream_model(&ctx.request.model);
```

`AnthropicCompatibleUpstream` does not — it forwards `ctx.raw_body` unchanged, so the original model string from the client is sent to the upstream.

### Impact
- Any route that configures `upstream_model` for a minimax/Anthropic-compatible provider has that field silently ignored
- The intended cheap minimax model is never substituted; the request may fail or route incorrectly at the minimax endpoint depending on what model strings minimax accepts

### Fix
`AnthropicCompatibleUpstream::proxy()` must deserialize the request, substitute `model` with `effective_upstream_model`, re-serialize, and send the modified body. The actual model from the upstream response should then be parsed and returned to the caller for DB storage.

### Files
- `crates/entry/api/src/services/gateway/upstream.rs` — `AnthropicCompatibleUpstream::proxy()`

---

## Summary of gaps by data flow

```
Client request
    │
    ▼
Gateway handler (messages.rs)
    │ ctx.model = requested model ← BUG 2 (should be upstream model)
    │ trace_id generated but unused ← BUG 4
    ▼
GatewayAudit::open()
    │ Writes ai_requests row
    │ Writes ai_request_payloads.request_body (raw JSON)
    │ MISSING: ai_request_messages rows ← BUG 3
    ▼
AnthropicCompatibleUpstream::proxy()
    │ Forwards raw_body unchanged ← BUG 6 (ignores upstream_model)
    │ Sends "claude-sonnet-4-6" to api.minimax.io
    ▼
Upstream response
    │ response.model (actual minimax model) — never read
    ▼
GatewayAudit::complete()
    │ Calculates cost using pricing::lookup("minimax", "claude-sonnet-4-6")
    │ "claude-sonnet-4-6" never matches minimax-m prefix ← BUG 1
    │ Falls through to _ arm at $0.2/$1.1 (wrong rates) ← BUG 1
    │ Writes ai_request_payloads.response_body
    │ MISSING: trace events ← BUG 4
    │ MISSING: conversation upsert ← BUG 5
    ▼
DB: ai_requests (complete, trace_id set)
    ai_request_payloads (request + response raw JSON, not surfaced by CLI)
    ai_request_messages (EMPTY — never written) ← BUG 3
    trace events (EMPTY — never written) ← BUG 4
    conversations (EMPTY — never written) ← BUG 5
```

---

## Fix priority

| # | Bug | Severity | Blocks |
|---|-----|----------|--------|
| 6 | `AnthropicCompatibleUpstream` ignores `upstream_model` | Critical | Bug 1, Bug 2 |
| 2 | Stored model is requested, not upstream | High | Bug 1 |
| 1 | Minimax pricing rates wrong / unreachable branch | High | cost reporting |
| 3 | Messages never written to `ai_request_messages` | High | audit usability |
| 4 | No trace events emitted from gateway path | Medium | trace tooling |
| 5 | Conversations table not updated from gateway | Medium | analytics |
