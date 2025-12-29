# systemprompt-client

HTTP client library for the SystemPrompt API.

## Purpose

Provides a clean HTTP interface for communicating with the SystemPrompt API server. Designed to decouple the TUI module from business logic dependencies by providing all server communication through HTTP.

## Structure

```
src/
├── lib.rs      # Public exports
├── client.rs   # SystempromptClient struct and API methods
├── error.rs    # ClientError enum with thiserror
└── http.rs     # Internal HTTP helper functions
```

## Usage

```rust
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;

let client = SystempromptClient::new("https://api.example.com")?
    .with_token(token);

let agents = client.list_agents().await?;
let contexts = client.list_contexts().await?;
```

## API Operations

| Category | Methods |
|----------|---------|
| Agents | `list_agents`, `get_agent_card` |
| Contexts | `list_contexts`, `get_context`, `create_context`, `delete_context`, `update_context_name`, `fetch_or_create_context`, `create_context_auto_name` |
| Tasks | `list_tasks`, `delete_task` |
| Artifacts | `list_artifacts`, `list_all_artifacts` |
| Messages | `send_message` |
| Admin | `list_logs`, `list_users`, `get_analytics` |
| Health | `check_health`, `verify_token` |

## Error Handling

All methods return `ClientResult<T>` which is `Result<T, ClientError>`. The `ClientError` enum provides typed errors:

- `HttpError` - Network/request failures
- `ApiError` - Server returned error response
- `JsonError` - Response parsing failed
- `AuthError` - Authentication issues
- `NotFound` - Resource not found
- `Timeout` - Request timeout
- `ServerUnavailable` - Server unreachable

## Dependencies

- `reqwest` - HTTP client
- `systemprompt-models` - Shared types (AgentCard, Task, etc.)
- `systemprompt-identifiers` - Typed identifiers (ContextId, JwtToken)
- `chrono` - DateTime handling
- `serde_json` - JSON serialization
- `thiserror` - Error derive macro
- `anyhow` - Error wrapping
