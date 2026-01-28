<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-provider-contracts

Provider trait contracts for systemprompt.io - LLM, Tool, Job, Template, Component providers.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-provider-contracts.svg)](https://crates.io/crates/systemprompt-provider-contracts)
[![Documentation](https://docs.rs/systemprompt-provider-contracts/badge.svg)](https://docs.rs/systemprompt-provider-contracts)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

Defines the core provider trait contracts used throughout systemprompt.io. These traits establish the interface boundaries for LLM providers, tool executors, job runners, template providers, and component renderers. Implementations live in domain crates while contracts remain in the shared layer for maximum composability.

**Part of the Shared layer in the systemprompt.io architecture.**

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-provider-contracts = "0.0.1"
```

## Quick Example

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

## Core Types

| Type | Description |
|------|-------------|
| `LlmProvider` | Trait for LLM chat completions |
| `ToolProvider` | Trait for tool discovery and execution |
| `Job` | Trait for background job execution |
| `TemplateProvider` | Trait for template loading |
| `ComponentRenderer` | Trait for component rendering |
| `PageDataProvider` | Trait for page data injection |
| `TemplateDataExtender` | Trait for extending template context |

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

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
