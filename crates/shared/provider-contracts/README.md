# systemprompt-provider-contracts

[![Crates.io](https://img.shields.io/crates/v/systemprompt-provider-contracts.svg?style=flat-square)](https://crates.io/crates/systemprompt-provider-contracts)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-provider-contracts?style=flat-square)](https://docs.rs/systemprompt-provider-contracts)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Defines provider interfaces consumed across the workspace without introducing dependencies on domain implementations.

**Layer**: Shared, foundational types and traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

The contracts are here, in the shared layer. The implementations live in domain crates. That split is deliberate: an LLM provider, a tool executor, a job runner, or a page renderer is a swappable part behind a stable interface, so the code that consumes AI never binds to a vendor. Define the trait once, implement it many times, choose the implementation from configuration.

## Architecture

| Module | Trait / Type | Purpose |
|--------|--------------|---------|
| `llm` | `LlmProvider`, `ToolExecutor`, `ChatRequest`, `ChatResponse`, `ChatStream` | LLM chat completions, streaming, and tool-call execution |
| `tool` | `ToolProvider`, `ToolDefinition`, `ToolCallRequest`, `ToolCallResult`, `ToolContent` | Tool discovery and invocation |
| `job` | `Job`, `JobContext`, `JobResult` | Background job execution |
| `template` | `TemplateProvider`, `TemplateDefinition`, `TemplateSource` | Template loading and resolution |
| `component` | `ComponentRenderer`, `ComponentContext`, `PartialTemplate`, `RenderedComponent` | Component rendering and partial sources |
| `page` | `PageDataProvider`, `PageContext` | Page data injection |
| `page_prerenderer` | `PagePrerenderer`, `PagePrepareContext`, `PageRenderSpec` | Static page prerendering |
| `extender` | `TemplateDataExtender`, `ExtenderContext`, `ExtendedData` | Template context extension |
| `frontmatter` | `FrontmatterProcessor`, `FrontmatterContext` | Frontmatter parsing and transformation |
| `content_data` | `ContentDataProvider`, `ContentDataContext` | Content data injection |
| `rss` | `RssFeedProvider`, `RssFeedSpec`, `RssFeedItem`, `RssFeedMetadata` | RSS feed generation |
| `sitemap` | `SitemapProvider`, `SitemapSourceSpec`, `SitemapUrlEntry`, `SitemapAlternate` | Sitemap generation with placeholder mapping |
| `web_config` | `WebConfig`, `BrandingConfig`, `ColorsConfig`, `TypographyConfig`, `LayoutConfig`, … | Declarative web/theme configuration loaded from YAML |
| `error` | `ProviderError`, `ProviderResult` | Shared error type for non-LLM, non-tool providers |

## Usage

```toml
[dependencies]
systemprompt-provider-contracts = "0.49"
```

```rust
use systemprompt_provider_contracts::llm::{
    ChatRequest, ChatResponse, ChatStream, LlmProvider, LlmProviderResult,
};
use async_trait::async_trait;

struct MyLlmProvider;

#[async_trait]
impl LlmProvider for MyLlmProvider {
    async fn chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatResponse> {
        todo!()
    }

    async fn stream_chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatStream> {
        todo!()
    }

    fn default_model(&self) -> &str {
        "my-model"
    }

    fn supports_model(&self, model: &str) -> bool {
        model == "my-model"
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn supports_tools(&self) -> bool {
        false
    }
}
```

```rust
use systemprompt_provider_contracts::web_config::WebConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = std::fs::read_to_string("services/web/config.yaml")?;
    let web: WebConfig = serde_yaml::from_str(&yaml)?;
    println!("site title: {}", web.branding.site_title);
    Ok(())
}
```

## Dependencies

### Internal

| Crate | Purpose |
|-------|---------|
| `systemprompt-identifiers` | Typed identifiers |

### External

| Crate | Purpose |
|-------|---------|
| `async-trait` | Async trait support for `dyn`-compatible providers |
| `futures` | Stream primitives for `ChatStream` |
| `inventory` | Compile-time provider registration |
| `serde`, `serde_json`, `serde_yaml` | Request/response and config (de)serialization |
| `thiserror` | Typed error enums |
| `chrono` | Timestamps in feed and sitemap entries |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
