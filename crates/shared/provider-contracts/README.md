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

# systemprompt-provider-contracts

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-provider-contracts — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-provider-contracts.svg?style=flat-square)](https://crates.io/crates/systemprompt-provider-contracts)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-provider-contracts?style=flat-square)](https://docs.rs/systemprompt-provider-contracts)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Provider trait contracts for systemprompt.io AI governance infrastructure. `LlmProvider`, `ToolProvider`, `JobContext`, and friends — swap Anthropic, OpenAI, Gemini, and local models at profile level. Implementations live in domain crates while contracts remain in the shared layer for maximum composability.

**Layer**: Shared — foundational types/traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Defines the core provider trait contracts used throughout systemprompt.io. These traits establish the interface boundaries for LLM providers, tool executors, job runners, template providers, and component renderers. Implementations live in domain crates while contracts remain in the shared layer for maximum composability.

## Architecture

| Type | Description |
|------|-------------|
| `LlmProvider` | Trait for LLM chat completions |
| `ToolProvider` | Trait for tool discovery and execution |
| `Job` | Trait for background job execution |
| `TemplateProvider` | Trait for template loading |
| `ComponentRenderer` | Trait for component rendering |
| `PageDataProvider` | Trait for page data injection |
| `TemplateDataExtender` | Trait for extending template context |

## Usage

```toml
[dependencies]
systemprompt-provider-contracts = "0.2.1"
```

```rust
use systemprompt_provider_contracts::{
    LlmProvider, ChatRequest, ChatResponse, LlmProviderResult,
    ToolProvider, ToolDefinition, ToolCallRequest, ToolCallResult,
};
use async_trait::async_trait;

struct MyLlmProvider;

#[async_trait]
impl LlmProvider for MyLlmProvider {
    async fn chat(&self, request: ChatRequest) -> LlmProviderResult<ChatResponse> {
        // Implementation
        todo!()
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
| `async-trait` | Async trait support |
| `inventory` | Compile-time registration |
| `serde` | Serialization |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-provider-contracts)** · **[docs.rs](https://docs.rs/systemprompt-provider-contracts)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
