# systemprompt-core-events Unit Tests

## Crate Overview
Event broadcasting and routing for real-time communication (SSE). Provides generic broadcasters for A2A, AgUI, and context events.

## Source Files
- `src/services/` - Event broadcasters and routers

## Test Plan

### Broadcaster Tests
- `test_broadcaster_register_connection` - Register new connection
- `test_broadcaster_unregister_connection` - Remove connection
- `test_broadcaster_broadcast_to_all` - Broadcast to all
- `test_broadcaster_connection_count` - Count connections

### Generic Broadcaster Tests
- `test_generic_broadcaster_typed_events` - Type-safe events
- `test_generic_broadcaster_multiple_types` - Multiple event types

### Event Router Tests
- `test_event_router_route_to_channel` - Route to channel
- `test_event_router_multiple_channels` - Multiple channels

### Connection Guard Tests
- `test_connection_guard_cleanup` - Automatic cleanup
- `test_connection_guard_drop` - Drop behavior

### Heartbeat Tests
- `test_heartbeat_interval` - Heartbeat timing
- `test_heartbeat_json_format` - Heartbeat format

## Mocking Requirements
- Mock SSE connections
- Mock event senders

## Test Fixtures Needed
- Sample events
- Sample connection IDs
