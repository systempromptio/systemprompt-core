# Configure AI providers

How to route AI inference through the gateway: define a provider catalog and routes in the services tree, point each at a provider's `base_url`, and serve client requests on `/v1/messages` and `/v1/models`. The gateway is the provider-facing proxy that sits between your AI clients and upstream model APIs (Anthropic, OpenAI, Google/Gemini, or any self-hosted compatible endpoint).

## Prerequisites

- A configured deployment per [configure.md](configure.md).
- An API key for each upstream provider you intend to route to.
- The provider's base URL (its OpenAI- or Anthropic-compatible API root).

## How the gateway routes a request

The gateway resolves an inbound request to one upstream in a fixed sequence (`crates/entry/api/src/services/gateway/service/mod.rs:49-160`):

1. A client `POST`s to `/v1/messages` (Anthropic wire format) or `/v1/responses` (OpenAI Responses wire format).
2. The gateway reads the requested `model` from the body and finds the first route whose `model_pattern` matches (`crates/shared/manifest/src/services/gateway/config/runtime.rs`).
3. It loads the route's API key from the secrets document by the `api_key_secret` name, and resolves the upstream adapter from the route's `provider` tag.
4. It applies gateway policy (allowed-model list) and per-user quota, then sends the request to the route's `endpoint`.
5. The upstream response is translated back into the inbound wire format and returned to the client.

The base path is `/v1` by default (`inference_path_prefix`, `gateway.rs:103-105`).

## 1. Enable the gateway

The provider catalog and the gateway are services-tree files, shipped with the deployment
rather than written per environment (`crates/shared/manifest/src/services/{providers,gateway}/`).
The gateway is disabled by default. Declare the catalog in `services/ai/providers.yaml`:

```yaml
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1
    api_key_secret: anthropic
    models:
      - id: claude-sonnet-4-5
        pricing: { input_per_million: 3.0, output_per_million: 15.0 }
  - name: openai
    wire: openai-chat
    surface: openai
    endpoint: https://api.openai.com/v1
    api_key_secret: openai
    models:
      - id: gpt-4o
```

and the routes in `services/ai/gateway.yaml`:

```yaml
gateway:
  enabled: true
  auth_scheme: bearer           # default; how clients present their token
  inference_path_prefix: /v1    # default; base path for /v1/messages, /v1/models
  routes:
    - model_pattern: claude-*
      provider: anthropic
    - model_pattern: gpt-*
      provider: openai
```

List both files under `includes:` in `services/config/config.yaml`. `systemprompt admin setup`
seeds them from the embedded default catalog for every provider whose key you supplied.

The registry is what `/v1/models` advertises and what validates each route's `provider`
reference; a route is dispatched only if a pattern matches the requested model. Credentials
are not in either file: `api_key_secret` names a key in the profile's secret store, so the
same catalog boots every environment.

## 2. Define routes

Each route maps a model name pattern to one upstream (`crates/shared/manifest/src/services/gateway/route.rs`):

| Field | Required | Meaning |
|-------|----------|---------|
| `model_pattern` | yes | Match against the requested model. `*` matches all; `prefix*` matches by prefix; `*suffix` by suffix; otherwise exact (`gateway.rs:267-278`). |
| `provider` | yes | The upstream adapter tag (see §3). |
| `endpoint` | yes | The provider base URL. Validated against the outbound-URL guard. |
| `api_key_secret` | yes | The key name in the secrets document holding this upstream's API key. |
| `upstream_model` | no | Send a different model name upstream than the client requested. |
| `extra_headers` | no | Additional headers added to the upstream request. The credential, content framing and protocol-version headers are reserved. |
| `accepted_betas` | no | The `anthropic-beta` values forwarded from a client request. Unset, first-party Anthropic forwards every beta and Vertex AI forwards none. The 1M context window is native on both and needs no beta. |
| `pricing` | no | Per-token pricing used for usage accounting. |
| `id` | no | Stable route id; synthesised from pattern/provider/endpoint if omitted. |
| `fallbacks` | no | Ordered list of `{provider, upstream_model}` deployments the request is re-sent to, in turn, when the previous one exhausts its transient-failure retries, returns a 5xx, or cannot be reached. A provider is only used when listed. |
| `by_scope` | no | `{dimension, chains: {<value>: {provider, upstream_model, fallbacks}}, unmapped: deny\|shared}`: requests attributed to a mapped value in that scope dimension use its own chain (for example a tenant's own Vertex project and region); an unmapped value is refused unless `unmapped: shared`. |
| `strategy` | no | `ordered` (default), `weighted` (first attempt drawn by each deployment's `weight`, default 1) or `least_busy` (first attempt to the deployment with the fewest in-flight requests in this process). Failover order after the first attempt is unchanged. |
| `context_fallbacks` | no | `{provider, upstream_model}` deployments with a larger context window. A request whose estimated input exceeds the selected model's `context_window` goes to the first that fits, or is refused with `context_window_exceeded` before any upstream call. |

The first matching route wins, so order specific patterns before general ones. The API key is referenced by name, not inlined: `api_key_secret: anthropic` reads the `anthropic` key from your secrets document (see [configure.md](configure.md) §5).

### Endpoint validation (SSRF guard)

Every provider `endpoint` is checked against the shared outbound-URL guard at load time (`ProviderRegistry::validate`, `crates/shared/manifest/src/services/providers/mod.rs`). An endpoint pointing at the loopback address, a link-local metadata address (`169.254.169.254`), or a private network range is rejected, so an operator-configured endpoint cannot turn the inference proxy into a server-side request forgery primitive. A self-hosted provider on a private network is therefore reachable only through an endpoint the guard accepts (a routable address or an approved egress proxy).

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

## Caller credentials: a member's own vendor account

A `kind: caller` credential serves an endpoint with the credential the client presents on each request, instead of a deployment secret or a stored member key. It is how a member's own subscription with a vendor is used through the gateway, for example the claude.ai login Claude Code holds: the request is authenticated, authorised, scanned, audited and counted here, and billed to the member by the vendor. Nothing changes for a deployment that does not declare one.

```yaml
# services/ai/credentials.yaml
credentials:
  claude-subscription: { kind: caller, label: Claude subscription }
```

```yaml
# services/ai/providers.yaml
providers:
  claude-subscription:
    credential: claude-subscription
    endpoints:
      api: { wire: anthropic, url: "https://api.anthropic.com/v1" }
```

```yaml
# services/ai/deployments.yaml
deployments:
  claude-opus-5-5: [claude-subscription/api]
```

A model bills one account on every deployment a request for it can reach. The catalog refuses a model whose enabled deployments mix a member-paid credential (`caller` or `personal`) with any other credential, and a route whose `context_fallbacks` name a model billed to a different credential than the models the route serves. An alias, a pinned route and a route selector all pick from the model's deployments, so the one rule covers them. To offer the same upstream model both ways, declare a second model id with `upstream_model` set to the vendor's id and deploy it on the caller provider.

The catalog refuses a caller credential that names a `secret`, that is used by more than one provider, whose provider sets `user_agent`, `extra_headers` or `forward_headers`, or whose provider has an endpoint that is not the vendor's own API (for example a model hosted on Vertex AI), that declares `accepted_betas`, or that is on a wire no client request format speaks (`gemini`).

**Request format.** A caller model is served only to a request sent in the format of its endpoint's wire, because the request reaches the upstream as the client built it: `/v1/messages` for `anthropic`, `/v1/chat/completions` for `openai-chat`, `/v1/responses` for `openai-responses`. A request in another format answers `400` with `invalid_request_error`, naming the model, the wire and the format that arrived, before the upstream is called. When a caller model has endpoints on more than one wire, the request is served by those on the wire it was sent in.

**Client.** The client keeps its vendor credential in `Authorization` and names its systemprompt credential in `x-systemprompt-authorization`. For Claude Code:

```bash
export ANTHROPIC_BASE_URL=https://<instance>/v1
export ANTHROPIC_CUSTOM_HEADERS="x-systemprompt-authorization: Bearer <PAT or JWT>"
```

Do not set `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY` or an `apiKeyHelper`: each replaces the claude.ai login as the credential Claude Code sends. The desktop bridge configures an `apiKeyHelper`, so a bridge-routed Claude Code does not carry a subscription and the bridge profile lists caller-credential models as not servable.

**Who may use it.** Deploy the subscription models on the caller provider under their own model ids or route, and grant that `gateway_route` or `model` entity to the users, departments or roles that may use their subscription in `access-control/rules.yaml`. Access is default-deny like every other route.

**What the gateway does on a caller hop.**

| Applied | Not applied |
|---------|-------------|
| Authentication, route and model authorisation, API-key model allowlists | System-prompt override rules |
| Quota and rate windows (tokens count; cost settles at zero) | Team-memory injection |
| Safety scanners and governance policies, which can deny | Secret sanitising and safety redaction: a prompt either would rewrite is denied instead |
| Audit journal and usage records (`billing_source = user_subscription`) | Body normalisation, automatic prompt caching, learned beta and field refusals |
| Response safety blocking of a buffered answer (`block_response_categories`) | Gateway retries: the request is sent once |
| Circuit breaker and `least_busy` load, scoped to the member | The platform's `traceparent` |
| | Response re-rendering and `stop_reason` correction |
| | Failover to any credential that bills a different account |

**The request.** The body is sent as the client sent it; the only edit is the `model` field when the route serves the model under a different upstream id, and a body that names the upstream id is sent byte for byte. The request carries the client's own headers and none of the platform's: `Authorization`, `user-agent` and the client's other headers are forwarded whole. Withheld are the systemprompt credential, every header and namespace the platform defines (`x-systemprompt-*`, session, context, task and trace identifiers, MCP and A2A session headers), any vendor API-key header (`x-api-key`, `x-goog-api-key`), cookies, client-address headers, trace context and this hop's framing headers. A request that reaches a caller hop without `x-systemprompt-authorization` and an `Authorization: Bearer` value answers `401`.

**The answer.** The upstream's answer is returned whole: its status, its body bytes (a stream is forwarded as it arrives, including frames the gateway does not recognise) and its headers, including rate-limit and `retry-after` headers. Withheld from the response are this hop's framing headers, `set-cookie` and any `x-systemprompt-*` name; the gateway adds `x-systemprompt-request-id`. A `429` or `5xx` is the upstream's own answer and is not retried by the gateway, so the client's retry policy and the vendor's limits apply unchanged. An answer whose body cannot be read whole is refused with a gateway error instead of being cut short. Usage for the audit row is read from a copy of the answer; a stream the gateway cannot decode is still delivered, and its row records the decode failure.

**Isolation between members.** A hop billed to a member keeps its circuit breaker and in-flight count per member, for `caller` and `personal` credentials alike: one member's exhausted limit or revoked key does not open the circuit, or change deployment selection, for anyone else.

**Adapters.** A caller hop is served by the built-in outbound adapter for its wire. An extension that registers an outbound adapter for a wire a caller credential uses stops the server at start, naming the wire.

**Anthropic's terms.** Whether a deployment may route subscription credentials through a gateway is governed by Anthropic's terms, not by this configuration. Anthropic's Claude Code documentation describes gateways that forward a claude.ai login ([LLM gateway](https://code.claude.com/docs/en/llm-gateway.md#subscriptions-and-gateways)), and its [legal and compliance](https://code.claude.com/docs/en/legal-and-compliance.md) page restricts third-party developers from offering claude.ai login in their own applications and from collecting, storing or intermediating claude.ai credentials; the [Agent SDK](https://code.claude.com/docs/en/agent-sdk/overview.md) documentation requires prior approval for third-party products that offer claude.ai login or rate limits. The gateway does not store the credential and does not modify the request. An operator who declares a caller credential is responsible for confirming that their use is permitted.

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
