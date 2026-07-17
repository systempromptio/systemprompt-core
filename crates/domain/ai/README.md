<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-ai

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-ai.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-ai.svg">
    <img alt="systemprompt-ai terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-ai.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-ai.svg?style=flat-square)](https://crates.io/crates/systemprompt-ai)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-ai?style=flat-square)](https://docs.rs/systemprompt-ai)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Every model call through one audited path. Anthropic, OpenAI, and Gemini answer to one governed pipeline, so the prompt, the tool call, the tokens, and the cost land in your database instead of a vendor's.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Integrations** · [Any AI Agent](https://systemprompt.io/features/any-ai-agent)

Multi-provider AI integration with tool execution, structured output, and request tracking.

This module provides a unified interface for AI model interactions across multiple providers (Anthropic, OpenAI, Gemini) with support for:

- Text generation and streaming
- Tool/function calling with MCP integration
- Structured JSON output with schema validation
- Image generation
- Google Search grounding
- Request tracking and cost estimation

## Architecture

This is an internal service library with no HTTP endpoints. AI capabilities are exposed through consuming modules (primarily the Agent module).

The AI module uses dependency injection for tool operations via the `ToolProvider` trait from `systemprompt-traits`. This allows:

- MCP module provides `McpToolProvider` for MCP-based tools
- AI module provides `NoopToolProvider` for non-tool use cases
- Other implementations possible without modifying AI module

## Usage

```toml
[dependencies]
systemprompt-ai = "0.21"
```

```rust
use systemprompt_ai::{AiService, AiRequest, AiMessage, NoopToolProvider};
use systemprompt_database::DbPool;
use std::sync::Arc;

let tool_provider = Arc::new(NoopToolProvider::new());
let ai_service = AiService::new(&db_pool, &registry, &ai_config, tool_provider, None)?;

let request = AiRequest::builder(
    vec![AiMessage::user("Hello!")],
    "gemini",
    "gemini-2.5-flash",
    8192,
    context,
)
.build();

let response = ai_service.generate(&request).await?;
```

## Module Layout

| Module | Purpose |
|--------|---------|
| `models/` | Unified `AiRequest`/`AiResponse` types plus provider-specific request and response DTOs for Anthropic, OpenAI, and Gemini. |
| `repository/` | Compile-time-verified persistence for requests, messages, tool calls, request/response payloads, quota buckets, gateway policies, and safety findings. |
| `services/core/` | `AiService` (top-level orchestration) and `ImageService`, with request storage and structured logging. |
| `services/providers/` | Per-provider implementations of the `AiProvider` trait for Anthropic, OpenAI, and Gemini, plus the image providers. |
| `services/gateway/` | Governance policy ingestion, safety scanning, route selection, and system-prompt overrides. Re-exported at the crate root. |
| `services/schema/` | Tool-schema transformation, including discriminated-union splitting for providers that reject `anyOf`. |
| `services/tooled/` | Tool-execution orchestration: runs calls through the injected `ToolProvider` and synthesizes responses. |
| `services/structured_output/` | JSON extraction and schema validation for structured output. |
| `services/storage/` | `ImageStorage` local blob storage for generated images. |
| `services/config/` | `ConfigValidator` for AI configuration. |

## Database

Tables: `ai_requests`, `ai_request_messages`, `ai_request_tool_calls`, `ai_request_payloads`, `ai_quota_buckets`, `ai_gateway_policies`, `ai_safety_findings`.

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-ai)** · **[docs.rs](https://docs.rs/systemprompt-ai)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
