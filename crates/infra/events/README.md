<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://systemprompt.io/documentation">Documentation</a> • <a href="https://github.com/systempromptio/systemprompt-core">Core</a> • <a href="https://github.com/systempromptio/systemprompt-template">Template</a></p>
</div>

---


# systemprompt-events

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../../assets/readme/terminals/dark/infra-events.svg">
    <source media="(prefers-color-scheme: light)" srcset="../../../assets/readme/terminals/light/infra-events.svg">
    <img alt="systemprompt-events terminal demo" src="../../../assets/readme/terminals/dark/infra-events.svg" width="100%">
  </picture>
</div>

Events module for systemprompt.io - event broadcasting and routing.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-events.svg)](https://crates.io/crates/systemprompt-events)
[![Documentation](https://docs.rs/systemprompt-events/badge.svg)](https://docs.rs/systemprompt-events)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

## Overview

**Part of the Infra layer in the systemprompt.io architecture.**

This crate provides a type-safe, generic event broadcasting system for real-time communication with connected clients via SSE (Server-Sent Events). It manages connection lifecycles, routes events to appropriate channels, and handles automatic cleanup of disconnected clients.

## File Structure

```
crates/infra/events/
├── Cargo.toml
├── README.md
├── status.md
└── src/
    ├── lib.rs                      # 27 lines  - Trait definitions, type aliases, re-exports
    └── services/
        ├── mod.rs                  # 10 lines  - Module re-exports
        ├── broadcaster.rs          # 191 lines - GenericBroadcaster implementation
        └── routing.rs              # 51 lines  - EventRouter, global singletons
```

## Modules

### `lib.rs`
Entry point defining core abstractions:
- `Broadcaster` trait - Type-safe async broadcasting with connection management
- `EventSender` type alias - Channel sender for SSE events (`UnboundedSender<Result<Event, Infallible>>`)

### `services/broadcaster.rs`
Generic broadcaster implementation:
- `GenericBroadcaster<E>` - Thread-safe broadcaster using `Arc<RwLock<HashMap<UserId, HashMap<ConnId, Sender>>>>`
- `ConnectionGuard<E>` - RAII guard for automatic connection cleanup on drop
- Type aliases: `AgUiBroadcaster`, `A2ABroadcaster`, `ContextBroadcaster`, `AnalyticsBroadcaster`
- Keep-alive utilities: `standard_keep_alive()`, `HEARTBEAT_INTERVAL`, `HEARTBEAT_JSON`

### `services/routing.rs`
Event routing and global state:
- `EventRouter` - Routes events to appropriate broadcaster(s)
- Global singletons: `AGUI_BROADCASTER`, `A2A_BROADCASTER`, `CONTEXT_BROADCASTER`, `ANALYTICS_BROADCASTER`

## Public API

### Traits
| Trait | Methods | Purpose |
|-------|---------|---------|
| `Broadcaster` | `register`, `unregister`, `broadcast`, `connection_count`, `total_connections` | Type-safe event broadcasting |

### Types
| Type | Description |
|------|-------------|
| `EventSender` | `UnboundedSender<Result<Event, Infallible>>` |
| `GenericBroadcaster<E>` | Generic broadcaster for any `ToSse + Clone + Send + Sync` event |
| `AgUiBroadcaster` | `GenericBroadcaster<AgUiEvent>` |
| `A2ABroadcaster` | `GenericBroadcaster<A2AEvent>` |
| `ContextBroadcaster` | `GenericBroadcaster<ContextEvent>` |
| `AnalyticsBroadcaster` | `GenericBroadcaster<AnalyticsEvent>` |
| `ConnectionGuard<E>` | RAII guard for automatic unregistration |
| `EventRouter` | Routes events to appropriate broadcasters |

### Constants
| Constant | Value | Purpose |
|----------|-------|---------|
| `HEARTBEAT_INTERVAL` | 15 seconds | SSE keep-alive interval |
| `HEARTBEAT_JSON` | `{"type":"heartbeat"}` | Keep-alive payload |

### Global Singletons
| Static | Type | Purpose |
|--------|------|---------|
| `AGUI_BROADCASTER` | `LazyLock<AgUiBroadcaster>` | AG-UI event broadcasts |
| `A2A_BROADCASTER` | `LazyLock<A2ABroadcaster>` | Agent-to-agent events |
| `CONTEXT_BROADCASTER` | `LazyLock<ContextBroadcaster>` | Aggregated context events |
| `ANALYTICS_BROADCASTER` | `LazyLock<AnalyticsBroadcaster>` | Analytics event tracking |

## Event Flow

```
                    ┌─────────────────┐
                    │   EventRouter   │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│AGUI_BROADCASTER│    │A2A_BROADCASTER│    │CONTEXT_BROADCASTER│
└───────┬───────┘    └───────┬───────┘    └───────┬───────┘
        │                    │                    │
        ▼                    ▼                    ▼
   SSE Clients          SSE Clients          SSE Clients
```

AgUI and A2A events are routed to both their primary broadcaster AND the context broadcaster for aggregation.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Event types (`AgUiEvent`, `A2AEvent`, `ContextEvent`, `SystemEvent`, `ToSse` trait) |
| `systemprompt-identifiers` | `UserId` type |
| `tokio` | Async runtime, channels, synchronization |
| `axum` | SSE `Event` and `KeepAlive` types |
| `async-trait` | Async trait support |
| `tracing` | Structured logging |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-events = "0.0.1"
```

## Tests

Tests are located in `crates/tests/unit/infra/events/` following the project convention of separating tests from source files.

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE) for details.
