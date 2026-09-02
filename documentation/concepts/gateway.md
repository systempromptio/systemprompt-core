# The provider-facing gateway

How systemprompt-core proxies model traffic to upstream providers: the gateway endpoints, request routing, and the controls — quota, policy, safety screening, and audit — applied to every proxied call.

The gateway is the provider-facing proxy. It accepts model requests on a stable, provider-shaped surface, screens and meters them, routes them to a configured upstream provider, and records a full audit trail. It is implemented in `crates/entry/api/src/routes/gateway` and `crates/entry/api/src/services/gateway`.

Note the distinction from the internal model path: the gateway proxy is a self-contained subsystem with its own outbound HTTP adapters. It does **not** share code with the internal AI service in `crates/domain/ai` that agents use, and in particular it does not inherit that path's `ResilientProvider` retry/circuit-breaker policy (see [The resilience boundary](#the-resilience-boundary)).

## The gateway surface

The gateway mounts under the base path `/v1`:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/messages` | POST | Anthropic-shaped messages request (inbound-adapted) |
| `/v1/responses` | POST | OpenAI-responses-shaped request (inbound-adapted) |
| `/v1/models` | GET | List available models from the catalog |
| `/v1/otel` (and `/v1/otel/{*rest}`) | POST | OTLP ingest of traces/logs/metrics from clients |

The two message endpoints accept different provider request shapes through inbound adapters (`AnthropicMessagesInbound`, `OpenAiResponsesInbound`) and converge on the same internal handler, so a caller can speak the request dialect it already knows. Every gateway request passes through an access-logging middleware that records method, path, status, and elapsed time both to `tracing` and to the `logs` table.

Request and response schemas for these endpoints belong in the reference material.

## Routing and the catalog

A gateway request names a model; the gateway resolves it to a configured route and dispatches the call. Routes and their upstream providers are configured per profile under the `gateway` section and resolved by the gateway registry (`crates/entry/api/src/services/gateway/registry.rs`). Each route names an upstream protocol — Anthropic Messages, OpenAI Chat Completions, or OpenAI Responses — handled by the matching outbound adapter under `crates/entry/api/src/services/gateway/protocol/outbound/`. The `/v1/models` endpoint surfaces the catalog of models the configured routes expose. A request that names an unconfigured model is rejected rather than dispatched.

## What the gateway enforces

Each proxied request passes through a fixed sequence of gateway-owned controls before and after the upstream call:

| Control | Behaviour | Source |
|---------|-----------|--------|
| Quota | Per-user/token usage metering; a request over budget is rejected before dispatch. | `services/gateway/quota.rs` |
| Policy | Request admissibility checks against the configured gateway policy. | `services/gateway/policy.rs` |
| Safety | Heuristic content screening on the request. | `services/gateway/safety/` |
| Audit | Every request and the streamed/whole response are recorded (method, path, status, latency, token counts, pricing). | `services/gateway/audit/`, `stream_tap/`, `pricing.rs` |
| SSRF guard | Outbound route endpoints are validated by the shared `validate_outbound_url` guard. | `crates/shared/models/src/net.rs` |

## The resilience boundary

This is the one property an operator must not misread. The gateway proxy's outbound adapters issue upstream calls with a request-scoped `reqwest::Client::new()` (`services/gateway/protocol/outbound/{anthropic,openai_chat,openai_responses}/mod.rs`). That client carries **no** retry, circuit breaker, bulkhead, or — currently — explicit request timeout. The gateway relies on per-user quota and the upstream's own behaviour, not on a resilience decorator.

The timeout/retry/circuit-breaker/bulkhead policy described elsewhere belongs to the **internal** AI service path: `ProviderFactory::create` wraps every provider built for `crates/domain/ai` in a `ResilientProvider` decorator (`crates/domain/ai/src/services/providers/provider_factory.rs:81`). That path serves internal callers such as agents — it is not on the `/v1/*` proxy route. Treat the two paths as having different reliability characteristics: a slow or failing upstream reached through the gateway proxy is not retried or circuit-broken, and (absent an operator-imposed timeout at the reverse proxy) is not time-bounded.

## See also

- [a2a-protocol.md](a2a-protocol.md) — agents are the primary internal consumers of the gateway.
- [architecture.md](architecture.md) — where the AI domain and the entry-layer gateway routes sit in the layering.
- [The stability contract](../security/stability-contract.md) for the surface's compatibility guarantees.
