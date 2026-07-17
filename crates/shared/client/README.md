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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The outside edge of a systemprompt.io deployment. Every CLI and external service reaches the server through one typed HTTP surface, never a repository or a database. What runs the deployment stays behind the API, and callers hold requests, not internals.

**Layer**: Shared, foundational types and traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

`SystempromptClient` wraps a pre-configured `reqwest::Client` and exposes typed methods for the routes declared in `systemprompt-models::ApiPaths`. External applications talk to a running deployment without linking against its business logic. Requests carry typed identifiers (`ContextId`, `JwtToken`), responses deserialize into shared models, and failures resolve to a single `ClientError` enum.

`RemoteCliExecutor` streams remote CLI command output over server-sent events into a caller-supplied `OutputSink`, so a local shell can drive a command running on the deployment and see its output arrive line by line.

## Module map

| Module | Responsibility |
|--------|----------------|
| `client` | `SystempromptClient` and every typed API method; `client/http.rs` holds the internal GET/POST/PUT/DELETE helpers |
| `error` | `ClientError` enum and the `ClientResult<T>` alias |
| `remote_cli` | `RemoteCliExecutor`, `RemoteCliRequest`, and the `OutputSink` trait for SSE-streamed CLI output |

### Error model

| Variant | Meaning |
|---------|---------|
| `HttpError` | Network or transport failure (wraps `reqwest::Error`) |
| `ApiError` | Server returned a non-2xx response, carrying `status`, `message`, `details` |
| `JsonError` | Response body failed to deserialize |
| `AuthError` | Authentication or authorization failure |
| `NotFound` | Requested resource does not exist |
| `Timeout` | Request exceeded its time limit |
| `ServerUnavailable` | Server unreachable |
| `ConfigError` | Invalid client configuration |
| `EventStreamSetup` | Failed to open a server-sent event stream |
| `Io` | Local I/O failure (wraps `std::io::Error`) |

`is_retryable()` returns true for `Timeout`, `ServerUnavailable`, and `HttpError`.

## Usage

```toml
[dependencies]
systemprompt-client = "0.21"
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
        Err(ClientError::AuthError { message }) => {
            eprintln!("Authentication failed: {}", message);
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
| `reqwest-eventsource` | Server-sent event streams for remote CLI output |
| `futures` | Async combinator utilities |
| `serde` / `serde_json` | JSON serialization |
| `chrono` | DateTime handling for auto-naming |
| `thiserror` | Derive macro for error types |
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
