# systemprompt-gateway

[![Crates.io](https://img.shields.io/crates/v/systemprompt-gateway.svg?style=flat-square)](https://crates.io/crates/systemprompt-gateway)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-gateway?style=flat-square)](https://docs.rs/systemprompt-gateway)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The AI gateway: a protocol-translating proxy in front of upstream LLM providers. A request in one wire protocol (Anthropic Messages, OpenAI Chat Completions or Responses) is parsed into the canonical model, governed by the installation's gateway policies, dispatched to an upstream provider with failover, and rendered back in the caller's protocol, with every request recorded in the AI-request audit trail.

**Layer**: App, orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace. The crate owns no HTTP surface: `systemprompt-api` mounts the gateway routes and calls `GatewayService`.

## Installation

```toml
[dependencies]
systemprompt-gateway = "0.65"
```

## Module map

| Module | Contents |
|--------|----------|
| `service` | `GatewayService`: route resolution, governance stages, outbound dispatch, failover, finalisation and pricing. |
| `protocol` | Inbound adapters (Anthropic Messages, OpenAI Chat, OpenAI Responses) and outbound adapters (Anthropic, OpenAI Chat, OpenAI Responses, Gemini) over the canonical model from `systemprompt-wire`. |
| `policies` | `GatewayPolicySpec`, the `policies.yaml` loader and ingestion, `PolicyResolver`, and the safety-scanner, route-selector and system-prompt-override extension contracts (`register_safety_scanner!`, `register_route_selector!`, `register_system_prompt_override!`). |
| `quota` | Per-subject quota windows: pre-check, reservation and post-update. |
| `registry` | Upstream and safety-scanner registries resolved from inventory registrations. |
| `audit` | `GatewayAudit` request lifecycle, the encrypted settlement journal and the access-log terminal record. |
| `stream_tap` | Streaming accumulation, terminal accounting and abort handling. |
| `image_fetch` | Guarded remote image fetch for providers that need inline data. |
| `signature_cache` | Thought-signature cache for reasoning replay. |
| `repository` | `GatewayRepositories`, the repository bundle built once at the gateway composition root. |
| `error` | `GatewayAuditError`. |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).
