# Configure AI providers

How to route AI inference through the gateway: define a provider catalog and routes in the profile, point each at a provider's `base_url`, and serve client requests on `/v1/messages` and `/v1/models`. The gateway is the provider-facing proxy that sits between your AI clients and upstream model APIs (Anthropic, OpenAI, Google/Gemini, or any self-hosted compatible endpoint).

## Prerequisites

- A configured deployment per [configure.md](configure.md).
- An API key for each upstream provider you intend to route to.
- The provider's base URL (its OpenAI- or Anthropic-compatible API root).

## How the gateway routes a request

The gateway resolves an inbound request to one upstream in a fixed sequence (`crates/entry/api/src/services/gateway/service/mod.rs:49-160`):

1. A client `POST`s to `/v1/messages` (Anthropic wire format) or `/v1/responses` (OpenAI Responses wire format).
2. The gateway reads the requested `model` from the body and finds the first route whose `model_pattern` matches (`crates/shared/models/src/profile/gateway.rs:108-110`).
3. It loads the route's API key from the secrets document by the `api_key_secret` name, and resolves the upstream adapter from the route's `provider` tag.
4. It applies gateway policy (allowed-model list) and per-user quota, then sends the request to the route's `endpoint`.
5. The upstream response is translated back into the inbound wire format and returned to the client.

The base path is `/v1` by default (`inference_path_prefix`, `gateway.rs:103-105`).

## 1. Enable the gateway

The gateway is configured under the profile's `gateway` section and is disabled by default (`crates/shared/models/src/profile/gateway.rs:86-97`). Enable it and declare the catalog and routes:

```yaml
gateway:
  enabled: true
  auth_scheme: bearer           # default; how clients present their token
  inference_path_prefix: /v1    # default; base path for /v1/messages, /v1/models
  catalog:
    providers:
      - name: anthropic
        endpoint: https://api.anthropic.com/v1
        api_key_secret: anthropic
      - name: openai
        endpoint: https://api.openai.com/v1
        api_key_secret: openai
    models:
      - id: claude-sonnet-4-5
        provider: anthropic
        display_name: Claude Sonnet 4.5
      - id: gpt-4o
        provider: openai
  routes:
    - model_pattern: claude-*
      provider: anthropic
      endpoint: https://api.anthropic.com/v1
      api_key_secret: anthropic
    - model_pattern: gpt-*
      provider: openai
      endpoint: https://api.openai.com/v1
      api_key_secret: openai
```

The `catalog` is the model directory that `/v1/models` advertises and that validates each model's `provider` reference. The `routes` list is what actually resolves an inbound model to an upstream — a request is dispatched only if a route matches it. The two are independent: a model can appear in the catalog without a route (it lists but cannot be called) and a route can match models not in the catalog.

## 2. Define routes

Each route maps a model name pattern to one upstream (`crates/shared/models/src/profile/gateway.rs:184-199`):

| Field | Required | Meaning |
|-------|----------|---------|
| `model_pattern` | yes | Match against the requested model. `*` matches all; `prefix*` matches by prefix; `*suffix` by suffix; otherwise exact (`gateway.rs:267-278`). |
| `provider` | yes | The upstream adapter tag (see §3). |
| `endpoint` | yes | The provider base URL. Validated against the outbound-URL guard. |
| `api_key_secret` | yes | The key name in the secrets document holding this upstream's API key. |
| `upstream_model` | no | Send a different model name upstream than the client requested. |
| `extra_headers` | no | Additional headers added to the upstream request. |
| `pricing` | no | Per-token pricing used for usage accounting. |
| `id` | no | Stable route id; synthesised from pattern/provider/endpoint if omitted. |

The first matching route wins, so order specific patterns before general ones. The API key is referenced by name, not inlined: `api_key_secret: anthropic` reads the `anthropic` key from your secrets document (see [configure.md](configure.md) §5).

### Endpoint validation (SSRF guard)

Every `endpoint` — in both `routes` and `catalog.providers` — is checked against the shared outbound-URL guard at load time (`crates/shared/models/src/profile/gateway.rs:57-65`). An endpoint pointing at the loopback address, a link-local metadata address (`169.254.169.254`), or a private network range is rejected, so an operator-configured endpoint cannot turn the inference proxy into a server-side request forgery primitive. A self-hosted provider on a private network is therefore reachable only through an endpoint the guard accepts (a routable address or an approved egress proxy).

## 3. Provider adapters and wire compatibility

The `provider` tag selects an outbound adapter that knows the upstream wire protocol. The built-in tags map to three adapters (`crates/entry/api/src/services/gateway/registry.rs:36-47`):

| `provider` tag | Outbound adapter | Upstream API shape | Path appended to `endpoint` |
|----------------|------------------|--------------------|------------------------------|
| `anthropic` | Anthropic | Anthropic Messages | `/messages` |
| `minimax` | Anthropic | Anthropic-compatible | `/messages` |
| `openai` | OpenAI Chat | OpenAI Chat Completions | `/chat/completions` |
| `moonshot` | OpenAI Chat | OpenAI-compatible | `/chat/completions` |
| `qwen` | OpenAI Chat | OpenAI-compatible | `/chat/completions` |
| `openai-responses` | OpenAI Responses | OpenAI Responses | (Responses path) |

The adapter appends the wire-specific path to the route's `endpoint`, so set `endpoint` to the API root (for Anthropic, `https://api.anthropic.com/v1`; the adapter forms `…/v1/messages`).

**Google/Gemini has no gateway outbound adapter.** The registry ships only the Anthropic and OpenAI-shaped adapters above. Gemini is supported through the platform's internal AI service path (`crates/domain/ai/src/services/providers/gemini/`), not through a gateway `provider` tag. To proxy a Gemini-backed model through `/v1/messages`, front it with an OpenAI- or Anthropic-compatible shim and route to that shim with the matching `provider` tag, or expose it through the internal AI service rather than the gateway.

**Self-hosted and other providers.** Any upstream that speaks the OpenAI Chat Completions or Anthropic Messages wire format works by setting `provider` to `openai` or `anthropic` and pointing `endpoint` at your host. For a wire format none of the built-in adapters cover, register a custom outbound adapter as an extension — the registry collects `OutboundAdapterRegistration` entries by tag at startup (`crates/entry/api/src/services/gateway/registry.rs:49-58`); a registered tag that collides with a built-in is logged.

## 4. Provide the API keys as secrets

Each route's `api_key_secret` names a key in the secrets document, not a literal. Add the keys your routes reference:

```json
{
  "anthropic": "sk-ant-...",
  "openai": "sk-...",
  "moonshot": "..."
}
```

A request whose route names a secret that is absent fails with a gateway error and the secret name in the message. Manage these keys through your secrets store as described in [configure.md](configure.md) §5; do not commit them.

## 5. Serve and verify

With the gateway enabled, two client-facing endpoints are live under `/v1`:

- `GET /v1/models` — lists the catalog models in OpenAI list shape (`crates/entry/api/src/routes/gateway/models.rs:41-84`). Returns `404` if the gateway is disabled.
- `POST /v1/messages` — accepts the Anthropic Messages wire format; `POST /v1/responses` accepts the OpenAI Responses format.

```bash
# List configured models
curl -fsS -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/v1/models

# Send a message; the body's "model" selects the route
curl -fsS -H "Authorization: Bearer $TOKEN" \
  -H "content-type: application/json" \
  http://127.0.0.1:8080/v1/messages \
  -d '{
        "model": "claude-sonnet-4-5",
        "max_tokens": 256,
        "messages": [{"role": "user", "content": "ping"}]
      }'
```

Each response carries an `x-systemprompt-request-id` header for correlation with the gateway access log and audit row.

## 6. Resilience behaviour

The platform's AI provider layer wraps every provider built through the provider factory in a single composed policy — per-attempt timeout, bounded retry with exponential backoff and jitter, a circuit breaker, and a bulkhead concurrency cap — applied uniformly (`crates/domain/ai/src/services/providers/provider_factory.rs:81-86`, `resilient_provider.rs`). Transient upstream failures (HTTP 408/425/429/5xx and transport timeouts) are retried with `Retry-After` honoured as a backoff floor; permanent failures fail fast but still count toward the breaker. This policy governs the platform's internal AI service path (used by agents and the AI domain).

The gateway proxy path is separate. The outbound adapters at `/v1/messages` send to the upstream with a request-scoped HTTP client and surface a non-2xx upstream as a `502`, with quota exhaustion as a `429` carrying `retry-after` (`crates/entry/api/src/services/gateway/protocol/outbound/`, `messages/dispatch.rs:70-82`); they do not layer the AI service's retry/circuit-breaker/bulkhead policy on each proxied call. Plan upstream retry and backoff for gateway clients accordingly, and rely on the per-user quota windows for gateway-side rate control.

## Next steps

- [configure.md](configure.md) — the profile and secrets that hold these settings.
- [operate.md](operate.md) — gateway error troubleshooting, metrics, and logs.
- [authoring-extensions.md](authoring-extensions.md) — register a custom outbound adapter for a new wire format.
