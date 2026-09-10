# The provider-facing gateway

How systemprompt-core proxies model traffic to upstream providers: the gateway endpoints, request routing, and the controls — quota, policy, safety screening, and audit — applied to every proxied call.

The gateway is the provider-facing proxy. It accepts model requests on a stable, provider-shaped surface, screens and meters them, routes them to a configured upstream provider, and records a full audit trail. It is implemented in `crates/entry/api/src/routes/gateway` and `crates/entry/api/src/services/gateway`.

Note the distinction from the internal model path: the gateway proxy is a self-contained subsystem with its own outbound HTTP adapters. It shares canonical wire types and codecs with the internal AI service in `crates/domain/ai`, but does not inherit that path's `ResilientProvider` policy (see [The resilience boundary](#resilience)).

## The gateway surface

The gateway mounts under the base path `/v1`:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/messages` | POST | Anthropic-shaped messages request (inbound-adapted) |
| `/v1/responses` | POST | OpenAI-responses-shaped request (inbound-adapted) |
| `/v1/chat/completions` | POST | OpenAI Chat Completions request |
| `/v1/models` | GET | List available models from the catalog |
| `/v1/otel` (and `/v1/otel/{*rest}`) | POST | OTLP ingest of traces/logs/metrics from clients |

The inference endpoints accept provider request shapes through inbound adapters and converge on the same internal handler, so a caller can speak the request dialect it already knows. Every gateway request passes through an access-logging middleware that records method, path, status, and elapsed time both to `tracing` and to the `logs` table.

Request and response schemas for these endpoints belong in the reference material.

## Routing and the catalog

A gateway request names a model; the gateway resolves it to a configured route and dispatches the call. Routes live in the services tree (`services/ai/gateway.yaml`, the `gateway:` key) and name providers declared in the services provider registry (`services/ai/providers.yaml`, the `providers:` key); both are loaded once at boot into `ServicesBootstrap` and resolved by the gateway registry (`crates/entry/api/src/services/gateway/registry.rs`). Each route references a provider whose wire protocol is Anthropic Messages, OpenAI Chat Completions, OpenAI Responses or Gemini — handled by the matching outbound adapter under `crates/entry/api/src/services/gateway/protocol/outbound/`. The `/v1/models` endpoint surfaces the catalog of models the configured routes expose. A request that names an unconfigured model is rejected rather than dispatched.

## What the gateway enforces

Each proxied request passes through a fixed sequence of gateway-owned controls before and after the upstream call:

| Control | Behaviour | Source |
|---------|-----------|--------|
| Quota | Subject-keyed usage windows; enforcing policies reject exhausted quotas. Cost is accounted after completion, so concurrent requests can exceed a ceiling. | `services/gateway/quota.rs` |
| Policy | Request admissibility checks against the configured gateway policy. | `services/gateway/policy.rs` |
| Safety | Heuristic content screening on the request. | `services/gateway/safety/` |
| Audit | Every request and the streamed/whole response are recorded (method, path, status, latency, token counts, pricing). | `services/gateway/audit/`, `stream_tap/`, `pricing.rs` |
| SSRF guard | Outbound route endpoints are validated by the shared `validate_outbound_url` guard. | `crates/shared/models/src/net/mod.rs` |

## Resilience

The gateway uses a shared outbound HTTP client and a bounded retry policy. Transient
HTTP 429 and 503 responses allow up to four total attempts before response bytes are
forwarded. Backoff starts at one second, caps its base delay at 30 seconds, adds jitter
and honors longer `retry-after` values. Transport failures and other statuses follow their
error paths. Attested evaluation dispatch disables retries. See
`crates/entry/api/src/services/gateway/protocol/outbound/retry.rs`.

The gateway does not inherit the internal AI service’s circuit-breaker and bulkhead
wrapper. Configure deployment-level concurrency and request-duration limits for the
expected streaming workload. Response-body failures after headers are sent are reported
through protocol-specific stream errors and terminal audit records.


## See also

- [a2a-protocol.md](a2a-protocol.md) — agents are the primary internal consumers of the gateway.
- [architecture.md](architecture.md) — where the AI domain and the entry-layer gateway routes sit in the layering.
- [The stability contract](../security/stability-contract.md) for the surface's compatibility guarantees.
