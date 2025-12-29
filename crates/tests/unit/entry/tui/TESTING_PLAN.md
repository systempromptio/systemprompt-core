# systemprompt-core-tui Unit Tests

## Crate Overview
Interactive terminal UI for chat-first AI interaction with real-time monitoring.

## Source Files
- `src/app/` - Main app logic
- `src/components/` - UI components
- `src/events/` - Event handling
- `src/services/` - Logging and utilities
- `src/state/` - Application state
- `src/tools/` - Tool integrations

## Test Plan

### Event Handling Tests
- `test_key_event_handling` - Keyboard events
- `test_mouse_event_handling` - Mouse events
- `test_resize_event_handling` - Resize events

### State Management Tests
- `test_state_initialization` - Initial state
- `test_state_transitions` - State changes
- `test_agent_state` - Agent state
- `test_chat_state` - Chat state

### Component Tests
- `test_agent_card_render` - Agent card
- `test_analytics_component` - Analytics display
- `test_artifact_renderers` - Artifact rendering
- `test_chat_component` - Chat display

### Message Routing Tests
- `test_message_handling` - Handle messages
- `test_message_routing` - Route messages

### Layout Tests
- `test_layout_calculation` - Calculate layout
- `test_layout_resize` - Handle resize

### Tool Integration Tests
- `test_tool_definition_loading` - Load tools
- `test_tool_execution` - Execute tools

## Mocking Requirements
- Mock terminal
- Mock event stream

## Test Fixtures Needed
- Sample events
- Sample state data
