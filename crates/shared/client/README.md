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

# systemprompt-client

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-client — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-client.svg?style=flat-square)](https://crates.io/crates/systemprompt-client)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-client?style=flat-square)](https://docs.rs/systemprompt-client)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

HTTP API client for systemprompt.io AI governance infrastructure. Typed requests for CLIs and external services talking to a systemprompt.io deployment. Communicates exclusively via HTTP — no direct access to repositories or services.

**Layer**: Shared — foundational types/traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides a type-safe, async HTTP client for interacting with the systemprompt.io API. It serves as the primary interface for external applications (CLI tools, third-party integrations) to communicate with the systemprompt.io server without depending on internal business logic.

### Design Principles

- **Decoupled Architecture**: Communicates exclusively via HTTP, no direct access to repositories or services
- **Type Safety**: Leverages `systemprompt-identifiers` for typed IDs (`ContextId`, `JwtToken`)
- **Error Transparency**: Structured error types with `thiserror` for clear failure modes
- **Async-First**: Built on `reqwest` and `tokio` for non-blocking I/O

## Architecture

```
src/
├── lib.rs        # Crate root - public API exports
├── client.rs     # SystempromptClient struct and all API methods
├── error.rs      # ClientError enum and ClientResult type alias
└── http.rs       # Internal HTTP helper functions (get, post, put, delete)
```

### `lib.rs`
Minimal public interface exposing only:
- `SystempromptClient` — the main client struct
- `ClientError` — error enum for all failure modes
- `ClientResult<T>` — type alias for `Result<T, ClientError>`

### `client.rs`
The core client implementation containing:
- **Construction**: `new()`, `with_timeout()`, `with_token()`
- **Token Management**: `set_token()`, `token()`
- **Agent Operations**: `list_agents()`, `get_agent_card()`
- **Context Operations**: `list_contexts()`, `get_context()`, `create_context()`, `delete_context()`, `update_context_name()`, `fetch_or_create_context()`, `create_context_auto_name()`
- **Task Operations**: `list_tasks()`, `delete_task()`
- **Artifact Operations**: `list_artifacts()`, `list_all_artifacts()`
- **Messaging**: `send_message()` (JSON-RPC format)
- **Admin Operations**: `list_logs()`, `list_users()`, `get_analytics()`
- **Health**: `check_health()`, `verify_token()`

### `error.rs`
Comprehensive error handling with variants:

| Variant | Description |
|---------|-------------|
| `HttpError` | Network/transport failures (wraps `reqwest::Error`) |
| `ApiError` | Server returned non-2xx response with status and body |
| `JsonError` | Response body failed to deserialize |
| `AuthError` | Authentication/authorization failures |
| `NotFound` | Requested resource does not exist |
| `Timeout` | Request exceeded time limit |
| `ServerUnavailable` | Server unreachable |
| `ConfigError` | Invalid client configuration |
| `Other` | Catch-all for unexpected errors |

Includes `is_retryable()` helper for retry logic.

### `http.rs`
Internal module providing low-level HTTP operations:
- `get<T>()` — GET request with optional auth, returns deserialized response
- `post<T, B>()` — POST with JSON body, returns deserialized response
- `put<B>()` — PUT with JSON body, returns unit
- `delete()` — DELETE request, returns unit

All functions handle authorization headers and error response parsing uniformly.

## Usage

```toml
[dependencies]
systemprompt-client = "0.2.1"
```

```rust
use systemprompt_client::{SystempromptClient, ClientResult};
use systemprompt_identifiers::JwtToken;

#[tokio::main]
async fn main() -> ClientResult<()> {
    // Create client with authentication
    let token = JwtToken::new("your-jwt-token");
    let client = SystempromptClient::new("https://api.systemprompt.io")?
        .with_token(token);

    // List available agents
    let agents = client.list_agents().await?;
    for agent in agents {
        println!("Agent: {}", agent.name);
    }

    // Work with contexts
    let contexts = client.list_contexts().await?;
    if let Some(ctx) = contexts.first() {
        let tasks = client.list_tasks(ctx.context_id.as_ref()).await?;
        println!("Found {} tasks", tasks.len());
    }

    // Health check (non-failing)
    if client.check_health().await {
        println!("Server is healthy");
    }

    Ok(())
}
```

```rust
use systemprompt_client::SystempromptClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SystempromptClient::new("http://localhost:9999")?;
    let agents = client.list_agents().await?;
    for agent in agents {
        println!("agent: {}", agent.name);
    }
    Ok(())
}
```

## API Operations

| Category | Methods |
|----------|---------|
| **Agents** | `list_agents`, `get_agent_card` |
| **Contexts** | `list_contexts`, `get_context`, `create_context`, `create_context_auto_name`, `fetch_or_create_context`, `update_context_name`, `delete_context` |
| **Tasks** | `list_tasks`, `delete_task` |
| **Artifacts** | `list_artifacts`, `list_all_artifacts` |
| **Messages** | `send_message` |
| **Admin** | `list_logs`, `list_users`, `get_analytics` |
| **Health** | `check_health`, `verify_token` |

## Error Handling

All fallible operations return `ClientResult<T>`. Pattern match on `ClientError` for specific handling:

```rust
use systemprompt_client::{ClientError, ClientResult};

async fn handle_errors(client: &SystempromptClient) -> ClientResult<()> {
    match client.list_agents().await {
        Ok(agents) => { /* success */ }
        Err(ClientError::AuthError(msg)) => {
            eprintln!("Authentication failed: {}", msg);
        }
        Err(ClientError::ApiError { status, message, .. }) => {
            eprintln!("API error {}: {}", status, message);
        }
        Err(e) if e.is_retryable() => {
            // Implement retry logic
        }
        Err(e) => return Err(e),
    }
    Ok(())
}
```

## Configuration

### Timeout
Default timeout is 30 seconds. Custom timeout:
```rust
let client = SystempromptClient::with_timeout("https://api.example.com", 60)?;
```

### Authentication
Token can be set at construction or later:
```rust
// At construction (builder pattern)
let client = SystempromptClient::new(url)?.with_token(token);

// After construction (mutation)
let mut client = SystempromptClient::new(url)?;
client.set_token(token);
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `reqwest` | HTTP client with async support |
| `tokio` | Async runtime |
| `serde` / `serde_json` | JSON serialization |
| `chrono` | DateTime handling for auto-naming |
| `thiserror` | Derive macro for error types |
| `anyhow` | Error wrapping for `Other` variant |
| `tracing` | Structured logging for error diagnostics |
| `systemprompt-models` | Shared API types (`AgentCard`, `Task`, etc.) |
| `systemprompt-identifiers` | Typed identifiers (`ContextId`, `JwtToken`) |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-client)** · **[docs.rs](https://docs.rs/systemprompt-client)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
