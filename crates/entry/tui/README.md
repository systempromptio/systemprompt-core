# systemprompt-core-tui

Terminal User Interface for SystemPrompt using ratatui with MVU architecture.

## Public API

- `TuiApp` - Main application entry point
- `TuiConfig` - Configuration for layout, theme, keybindings
- `Message` - MVU message type
- `init_file_logging()` - Initialize file logging
- `get_log_file_path()` - Get log file path

## Review Status

See [status.md](status.md) for the full code review results.

**Current Verdict:** REJECTED - Requires removal of business logic dependencies (see BD6/DD5)

## Structure

```
src/
├── lib.rs
│
├── app/
│   ├── mod.rs                 # TuiApp, event loop
│   ├── config.rs              # TuiConfig, LayoutConfig, ThemeConfig
│   ├── layout.rs              # AppLayout, centered_rect()
│   ├── messages.rs            # Message, Command, SlashCommand
│   ├── init.rs                # State initialization
│   ├── init_render.rs         # Startup rendering
│   ├── commands/
│   │   ├── mod.rs             # Command dispatch
│   │   ├── conversation.rs    # Conversation operations
│   │   └── sync.rs            # Sync operations
│   ├── draw/
│   │   ├── mod.rs             # Main draw loop
│   │   └── overlays.rs        # Modal rendering
│   └── update/
│       ├── mod.rs             # Update dispatch
│       ├── chat.rs            # Chat handlers
│       ├── domain.rs          # Domain handlers
│       ├── navigation.rs      # Navigation handlers
│       └── tools.rs           # Tool handlers
│
├── components/
│   ├── mod.rs
│   ├── agents.rs              # Agent list
│   ├── approval.rs            # Tool approval dialog
│   ├── artifacts.rs           # Artifact list
│   ├── config.rs              # Config display
│   ├── control_guide.rs       # Control help
│   ├── conversations.rs       # Conversation list
│   ├── global_input.rs        # Input field
│   ├── logs_tab.rs            # Logs panel
│   ├── shortcuts.rs           # Shortcuts display
│   ├── sidebar.rs             # Sidebar
│   ├── spinner.rs             # Loading spinner
│   ├── tabs.rs                # Tab bar
│   ├── users.rs               # User list
│   ├── agent_card/
│   │   ├── mod.rs
│   │   └── sections.rs
│   ├── analytics/
│   │   ├── mod.rs
│   │   └── tables.rs
│   ├── artifact_renderers/
│   │   ├── mod.rs
│   │   ├── card.rs
│   │   ├── dashboard.rs
│   │   ├── list.rs
│   │   ├── table.rs
│   │   └── text.rs
│   └── chat/
│       ├── mod.rs
│       ├── execution_timeline.rs
│       ├── input_request.rs
│       ├── message_list.rs
│       ├── task_detail.rs
│       └── tool_panel.rs
│
├── events/
│   ├── mod.rs
│   ├── handler.rs             # Event capture
│   ├── tui_event_bus.rs       # Event publishing
│   ├── tui_events.rs          # TuiEvent enum
│   └── keybindings/
│       ├── mod.rs
│       ├── tabs.rs
│       └── dialogs.rs
│
├── services/
│   ├── mod.rs
│   ├── agent/
│   │   ├── mod.rs
│   │   └── discovery.rs
│   ├── analytics/
│   │   ├── mod.rs
│   │   ├── poller.rs
│   │   └── subscriber.rs
│   ├── cloud/
│   │   ├── mod.rs
│   │   └── api.rs
│   ├── context/
│   │   ├── mod.rs
│   │   ├── service.rs
│   │   └── stream_subscriber.rs
│   ├── log/
│   │   ├── mod.rs
│   │   ├── streamer.rs
│   │   └── subscriber.rs
│   ├── logging/
│   │   └── mod.rs
│   ├── message/
│   │   ├── mod.rs
│   │   └── sender.rs
│   ├── service/
│   │   ├── mod.rs
│   │   └── subscriber.rs
│   └── user/
│       ├── mod.rs
│       ├── poller.rs
│       └── subscriber.rs
│
├── state/
│   ├── mod.rs                 # AppState
│   ├── app_state.rs
│   ├── agents.rs
│   ├── analytics.rs
│   ├── artifacts.rs
│   ├── commands.rs
│   ├── conversations.rs
│   ├── logs.rs
│   ├── services.rs
│   ├── tools.rs
│   ├── users.rs
│   └── chat/
│       ├── mod.rs             # ChatState
│       ├── messages.rs
│       ├── tasks.rs
│       ├── progress.rs
│       └── types.rs
│
└── tools/
    ├── mod.rs
    ├── executor.rs
    ├── registry.rs
    └── definitions/
        ├── mod.rs
        ├── logs.rs
        ├── database/
        │   ├── mod.rs
        │   ├── query.rs
        │   └── schema.rs
        └── services/
            ├── mod.rs
            ├── control.rs
            ├── list.rs
            └── status.rs
```
