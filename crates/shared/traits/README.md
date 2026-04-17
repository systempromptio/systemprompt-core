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

# systemprompt-traits

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-traits — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-traits.svg?style=flat-square)](https://crates.io/crates/systemprompt-traits)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-traits?style=flat-square)](https://docs.rs/systemprompt-traits)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Trait-first interface contracts for systemprompt.io AI governance infrastructure. Repository, provider, and service abstractions shared across the MCP governance pipeline. Provides the core trait definitions that enable polymorphism, dependency injection, and consistent patterns throughout the codebase.

**Layer**: Shared — foundational types/traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides the core trait definitions that enable polymorphism, dependency injection, and consistent patterns across the systemprompt.io codebase.

## Architecture

This crate follows the **Interface Segregation Principle** from SOLID:
- Traits are small and focused
- Clients depend only on the methods they use
- No fat interfaces or forced implementations

**No dependencies on other systemprompt.io crates** (except `systemprompt-provider-contracts` and `systemprompt-identifiers`) — intentional to prevent circular dependencies.

## Usage

```toml
[dependencies]
systemprompt-traits = "0.2.1"
```

```rust
use async_trait::async_trait;
use systemprompt_traits::Service;

struct HealthPinger;

#[async_trait]
impl Service for HealthPinger {
    fn name(&self) -> &str { "health-pinger" }
    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
    async fn health_check(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> { Ok(true) }
}
```

## Traits

### Repository Traits

**`Repository`** — Base trait for all repository implementations
```rust
use systemprompt_traits::{Repository, RepositoryError};

impl Repository for MyRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}
```

**`CrudRepository<T>`** — Generic CRUD operations trait
```rust
use systemprompt_traits::CrudRepository;

impl CrudRepository<User> for UserRepository {
    type Id = String;

    async fn create(&self, entity: User) -> Result<User, Self::Error> { ... }
    async fn get(&self, id: Self::Id) -> Result<Option<User>, Self::Error> { ... }
    async fn update(&self, entity: User) -> Result<User, Self::Error> { ... }
    async fn delete(&self, id: Self::Id) -> Result<(), Self::Error> { ... }
    async fn list(&self) -> Result<Vec<User>, Self::Error> { ... }
}
```

**`RepositoryError`** — Standard error type for repository operations
```rust
pub enum RepositoryError {
    DatabaseError(sqlx::Error),
    NotFound(String),
    SerializationError(serde_json::Error),
    InvalidData(String),
    ConstraintViolation(String),
    GenericError(anyhow::Error),
}
```

### Context Traits

**`AppContext`** — Application context trait for dependency injection
```rust
use systemprompt_traits::AppContext;

impl AppContext for MyAppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> { ... }
    fn module_registry(&self) -> Arc<dyn ModuleRegistry> { ... }
    fn database_handle(&self) -> Arc<dyn DatabaseHandle> { ... }
}
```

**`ConfigProvider`** — Configuration provider trait
```rust
impl ConfigProvider for Config {
    fn get(&self, key: &str) -> Option<String> { ... }
    fn database_url(&self) -> &str { ... }
    fn system_path(&self) -> &str { ... }
    fn jwt_secret(&self) -> &str { ... }
    fn api_port(&self) -> u16 { ... }
}
```

**`ModuleRegistry`** — Module registry trait for dynamic module management
```rust
impl ModuleRegistry for MyModuleRegistry {
    fn get_module(&self, name: &str) -> Option<Arc<dyn Module>> { ... }
    fn list_modules(&self) -> Vec<String> { ... }
}
```

### Service Traits

**`Service`** — Base service trait with lifecycle methods
```rust
use systemprompt_traits::Service;

#[async_trait]
impl Service for MyService {
    fn name(&self) -> &str { "my-service" }

    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
    async fn health_check(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> { ... }
}
```

**`AsyncService`** — Async service trait for long-running background tasks
```rust
use systemprompt_traits::AsyncService;

#[async_trait]
impl AsyncService for MyAsyncService {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Long-running task
    }
}
```

### Module Traits

**`Module`** — Core module trait for systemprompt.io modules
```rust
#[async_trait]
impl Module for MyModule {
    fn name(&self) -> &str { "my-module" }
    fn version(&self) -> &str { "1.0.0" }
    fn display_name(&self) -> &str { "My Module" }
    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> { ... }
}
```

**`ApiModule`** — Module trait with REST API support
```rust
#[async_trait]
impl ApiModule for MyApiModule {
    fn router(&self, ctx: Arc<dyn AppContext>) -> axum::Router { ... }
}
```

### Tool Provider Traits

**`ToolProvider`** — Abstract tool discovery and execution

Enables modules to use tools without depending on specific implementations (e.g., MCP).

```rust
use systemprompt_traits::{ToolProvider, ToolContext, ToolDefinition};

#[async_trait]
impl ToolProvider for MyToolProvider {
    async fn list_tools(&self, agent_name: &str, context: &ToolContext)
        -> ToolProviderResult<Vec<ToolDefinition>> { ... }
    async fn call_tool(&self, request: &ToolCallRequest, service_id: &str, context: &ToolContext)
        -> ToolProviderResult<ToolCallResult> { ... }
    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()> { ... }
    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> { ... }
}
```

Supporting types: `ToolDefinition`, `ToolCallRequest`, `ToolCallResult`, `ToolContent`, `ToolContext`, `ToolProviderError`

### LLM Provider Traits

**`LlmProvider`** — Abstract LLM interactions

```rust
use systemprompt_traits::{LlmProvider, ChatRequest, ChatResponse};

#[async_trait]
impl LlmProvider for MyProvider {
    async fn chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatResponse> { ... }
    async fn stream_chat(&self, request: &ChatRequest) -> LlmProviderResult<ChatStream> { ... }
    fn default_model(&self) -> &str { "model-name" }
    fn supports_model(&self, model: &str) -> bool { ... }
    fn supports_streaming(&self) -> bool { true }
    fn supports_tools(&self) -> bool { true }
}
```

**`ToolExecutor`** — Execute tools during conversations

```rust
use systemprompt_traits::{ToolExecutor, ToolExecutionContext};

#[async_trait]
impl ToolExecutor for MyExecutor {
    async fn execute(&self, tool_calls: Vec<ToolCallRequest>, tools: &[ToolDefinition],
        context: &ToolExecutionContext) -> (Vec<ToolCallRequest>, Vec<ToolCallResult>) { ... }
}
```

Supporting types: `ChatMessage`, `ChatRole`, `ChatRequest`, `ChatResponse`, `SamplingParameters`, `TokenUsage`, `ToolExecutionContext`

## Usage Patterns

### When to Use Traits vs Concrete Types

**Use Traits When:**
- You need dependency injection for testing
- You want to support multiple implementations
- You're defining interfaces between modules
- You need polymorphic behavior

**Use Concrete Types When:**
- Performance is critical and trait objects add overhead
- There's only one implementation
- The API is module-internal
- Type inference is important

### Testing with Traits

Traits enable easy mocking:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockRepository {
        pool: MockPool,
    }

    impl Repository for MockRepository {
        type Pool = MockPool;
        type Error = RepositoryError;

        fn pool(&self) -> &Self::Pool { &self.pool }
    }

    #[tokio::test]
    async fn test_with_mock() {
        let repo = MockRepository { pool: MockPool::new() };
        // Test using trait methods
    }
}
```

### Error Handling

All repository errors automatically convert to `ApiError`:

```rust
use systemprompt_models::{ApiError, RepositoryError};

let result: Result<User, RepositoryError> = repo.get_user("id").await;
let api_result: Result<User, ApiError> = result.map_err(|e| e.into());
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `web` | No | Axum router types for `ApiModule` trait |

## Dependencies

- `async-trait` — Async trait support
- `anyhow` — Error handling
- `axum` — Router type for ApiModule (optional, with `web` feature)
- `inventory` — Module registration
- `thiserror` — Error derive macros
- `serde_json` — Serialization errors
- `systemprompt-provider-contracts` — Provider trait definitions
- `systemprompt-identifiers` — Typed identifiers

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-traits)** · **[docs.rs](https://docs.rs/systemprompt-traits)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
