# systemprompt-wire

[![Crates.io](https://img.shields.io/crates/v/systemprompt-wire.svg?style=flat-square)](https://crates.io/crates/systemprompt-wire)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-wire?style=flat-square)](https://docs.rs/systemprompt-wire)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Defines the canonical AI request, response and event model and the per-protocol codecs that translate it to and from each upstream provider's wire format.

**Layer**: Shared. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace. Depends only on [`systemprompt-identifiers`](https://crates.io/crates/systemprompt-identifiers) among the workspace crates.

## Installation

```toml
[dependencies]
systemprompt-wire = "0.64"
```

## Module map

| Module | Contents |
|--------|----------|
| `canonical` | Provider-neutral `CanonicalRequest`, `CanonicalResponse`, `CanonicalEvent`, usage and safety types. |
| `anthropic` | Anthropic Messages request build, response parse, SSE translation, beta-header policy. |
| `openai_chat` | OpenAI Chat Completions request build, response parse and stream deltas. |
| `openai_responses` | OpenAI Responses request build, response parse and streaming events. |
| `gemini` | Gemini / Vertex AI request build, response parse, streaming and thinking budgets. |
| `upstream` | `UpstreamDialect`: headers and URL shape for a (wire protocol, hosting) pair. |
| `protocol` | `WireProtocol`, the request/response dialect a provider speaks. |
| `hosting` | `Hosting`, the platform in front of a provider (first party or Vertex AI). |
| `limits` | `ModelLimits`, the per-model token ceilings a codec clamps to. |
| `schema` | Per-provider JSON-Schema capability matrices and the tool-schema sanitiser. |
| `inspect` | Leaf-level request inspection used to prove what is sent upstream. |
| `sse` | Server-sent-event framing over byte streams. |
| `defect`, `error` | Typed codec defects and errors. |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).
