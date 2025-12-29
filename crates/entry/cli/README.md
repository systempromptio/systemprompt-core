# systemprompt-cli

**Layer:** Entry
**Binary:** `systemprompt`

Command-line interface for SystemPrompt OS - agent orchestration, AI operations, and system management.

## Structure

```
src/
├── main.rs
├── tui.rs
├── common/
│   ├── docker.rs
│   ├── paths.rs
│   └── process.rs
├── presentation/
│   ├── renderer.rs
│   ├── state.rs
│   └── widgets.rs
├── services/
│   ├── db/
│   └── scheduler/
├── cloud/
│   ├── config/
│   ├── setup/
│   │   └── boilerplate/templates/
│   └── sync/
├── agents/
│   └── mcp.rs
├── logs/
│   └── trace/
├── config/
│   └── profile/
└── build/
```

## Commands

```
systemprompt
systemprompt services [...]
systemprompt cloud [...]
systemprompt agents [...]
systemprompt logs [...]
systemprompt config [...]
systemprompt build [...]
```

## Standards

### Output

Use `CliService` for all output:

```rust
use systemprompt_core_logging::CliService;

CliService::section("Title");
CliService::info("message");
CliService::success("message");
CliService::warning("message");
CliService::error("message");
CliService::key_value("Key", "Value");
```

### Startup Display

Use shared display components from `systemprompt_core_logging::services::cli`:

```rust
use systemprompt_core_logging::services::cli::{
    render_startup_banner,
    render_phase_header,
    render_phase_success,
    render_service_table,
    render_startup_complete,
    BrandColors,
    ServiceTableEntry,
    ServiceStatus,
};
```

### Logging

Use `SystemSpan` for tracing context:

```rust
use systemprompt_core_logging::SystemSpan;

let _span = SystemSpan::new("cli");
```

### Command Pattern

Commands delegate to services:

```rust
pub async fn execute(cmd: Command) -> Result<()> {
    match cmd {
        Command::Start { flags } => services::start::execute(flags).await,
        Command::Stop { flags } => services::stop::execute(flags).await,
    }
}
```

## Forbidden

- `println!` - Use `CliService` methods
- `unwrap()` / `expect()` - Use `?` operator
- `panic!` - Return `Result` with context
- Comments - Code must be self-documenting
- Direct SQL in commands
- Business logic in handlers

## Dependencies

- `clap` - Command parsing with derive macros
- `indicatif` - Progress bars and spinners
- `dialoguer` - Interactive prompts
- `console` - Terminal styling
- `systemprompt-core-logging` - Shared display components
