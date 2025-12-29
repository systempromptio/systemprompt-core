# systemprompt-core-events

Event broadcasting and routing infrastructure for SSE connections.

## Directory Structure

```
src/
├── lib.rs                 # Exports, Broadcaster and EventBus traits
└── services/
    ├── mod.rs             # Re-exports from broadcaster.rs and routing.rs
    ├── broadcaster.rs     # GenericBroadcaster implementation, type aliases, ConnectionGuard
    └── routing.rs         # EventRouter, global broadcaster singletons
```

## Key Files

| File | Purpose |
|------|---------|
| `lib.rs` | Defines `Broadcaster` trait, `EventBus` trait, `EventSender` type alias |
| `services/broadcaster.rs` | `GenericBroadcaster<E>` struct with connection management, keep-alive utilities |
| `services/routing.rs` | `EventRouter` for dispatching events, global `Lazy` broadcaster instances |

## Exports

**Traits:**
- `Broadcaster` - Type-safe event broadcasting with connection management
- `EventBus` - Abstraction for event infrastructure (agui, a2a, system, context)

**Types:**
- `EventSender` - Channel sender for SSE events
- `GenericBroadcaster<E>` - Generic broadcaster implementation
- `AgUiBroadcaster`, `A2ABroadcaster`, `ContextBroadcaster` - Type aliases
- `ConnectionGuard<E>` - RAII guard for connection cleanup
- `EventRouter` - Routes events to appropriate broadcasters

**Globals:**
- `AGUI_BROADCASTER`, `A2A_BROADCASTER`, `CONTEXT_BROADCASTER` - Lazy singletons
- `HEARTBEAT_INTERVAL`, `HEARTBEAT_JSON` - Keep-alive constants

**Functions:**
- `standard_keep_alive()` - Returns configured `KeepAlive` for SSE streams

## Dependencies

- `systemprompt-models` - Event types (AgUiEvent, A2AEvent, ContextEvent, SystemEvent, ToSse)
- `systemprompt-identifiers` - UserId type
- `tokio`, `axum`, `async-trait`, `once_cell` - Runtime and framework
