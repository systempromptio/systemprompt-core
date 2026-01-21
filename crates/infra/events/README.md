# systemprompt-events

Event broadcasting and routing infrastructure for SSE (Server-Sent Events) connections.

## Overview

This crate provides a type-safe, generic event broadcasting system for real-time communication with connected clients. It manages connection lifecycles, routes events to appropriate channels, and handles automatic cleanup of disconnected clients.

## File Structure

```
crates/infra/events/
├── Cargo.toml
├── README.md
├── status.md
└── src/
    ├── lib.rs                      # 52 lines  - Trait definitions, type aliases, re-exports
    └── services/
        ├── mod.rs                  # 11 lines  - Module re-exports
        ├── broadcaster.rs          # 192 lines - GenericBroadcaster implementation (+413 lines tests)
        └── routing.rs              # 52 lines  - EventRouter, global singletons (+260 lines tests)
```

## Modules

### `lib.rs`
Entry point defining core abstractions:
- `Broadcaster` trait - Type-safe async broadcasting with connection management
- `EventBus` trait - High-level abstraction for multi-channel event dispatch
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
| `EventBus` | `broadcast_agui`, `broadcast_a2a`, `broadcast_system`, `broadcast_context` | Multi-channel dispatch |

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
│ AGUI_BROADCASTER │    │ A2A_BROADCASTER │    │CONTEXT_BROADCASTER│
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
