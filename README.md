# SystemPrompt Core

**Platform and framework for SystemPrompt OS**

## What is SystemPrompt Core?

SystemPrompt Core provides the foundational platform for building AI agent orchestration systems:

- **Agent Orchestration**: Multi-agent lifecycle management via A2A protocol
- **MCP Protocol**: Model Context Protocol server and client implementations
- **Database Abstraction**: Unified interface supporting SQLite and PostgreSQL
- **HTTP API Framework**: REST API with OAuth2 authentication
- **Extension System**: Extensible architecture with compile-time registration
- **Configuration**: Profile-based configuration with startup validation
- **Structured Logging**: Context-aware logging with request tracing

## Architecture

Core uses a **layered crate architecture** with strict dependency rules:

```
crates/
├── shared/           # Pure types, zero internal dependencies
│   ├── traits/       # Core trait definitions (LlmProvider, ToolProvider, Job)
│   ├── models/       # Data models, API types, config structs
│   ├── identifiers/  # Typed IDs (UserId, TaskId, etc.)
│   ├── client/       # HTTP client for external APIs
│   └── extension/    # Extension framework for customization
│
├── infra/            # Stateless infrastructure utilities
│   ├── database/     # SQLx abstraction, connection pooling
│   ├── events/       # Event bus, broadcasters, SSE
│   ├── security/     # JWT validation, token handling
│   ├── config/       # Configuration loading
│   ├── logging/      # Tracing setup, log sinks
│   ├── cloud/        # Cloud API, tenant management
│   └── loader/       # Module loader
│
├── domain/           # Bounded contexts with SQL + repos + services
│   ├── users/        # User identity (User, Role)
│   ├── oauth/        # Authentication (Token, Client, Grant, Session)
│   ├── files/        # File storage (File, FileMetadata)
│   ├── analytics/    # Metrics & tracking (Session, Event, Metric)
│   ├── content/      # Content management (Content, Category, Tag)
│   ├── ai/           # LLM integration (Request, Response, Provider)
│   ├── mcp/          # MCP protocol (Server, Tool, Deployment)
│   └── agent/        # A2A protocol (Agent, Task, Context, Skill)
│
├── app/              # Orchestration, no business logic
│   ├── runtime/      # AppContext, StartupValidator, lifecycle
│   ├── scheduler/    # Job scheduling, cron execution
│   ├── generator/    # Static site generation
│   └── sync/         # Sync services
│
└── entry/            # Entry points (binaries, public APIs)
    ├── api/          # HTTP gateway, route handlers, middleware
    ├── cli/          # Command-line interface
    └── tui/          # Terminal UI

systemprompt/         # Facade: Public API for external consumers
```

### Dependency Flow

```
FACADE (systemprompt) ← External consumers
        │
    ENTRY (api, cli, tui)
        │
    APP (runtime, scheduler, generator, sync)
        │
    DOMAIN (users, oauth, ai, agent, mcp, ...)
        │
    INFRA (database, events, security, config, logging, cloud)
        │
    SHARED (models, traits, identifiers, client, extension)
```

Domain crates cannot depend on each other - use traits or events for cross-domain communication.

## Distribution Model

**SystemPrompt Core is distributed via Git (not crates.io).**

This workspace contains 27+ interdependent crates that share a single version and are published together. This approach ensures version consistency and simplifies development.

## Installation

### Quick Start

Add to your `Cargo.toml`:

```toml
[workspace.dependencies]
# Import only the modules you need
systemprompt-models = { git = "https://github.com/systempromptio/systemprompt-core", tag = "v0.0.1" }
systemprompt-core-api = { git = "https://github.com/systempromptio/systemprompt-core", tag = "v0.0.1" }
systemprompt-core-database = { git = "https://github.com/systempromptio/systemprompt-core", tag = "v0.0.1" }
systemprompt-core-mcp = { git = "https://github.com/systempromptio/systemprompt-core", tag = "v0.0.1" }
systemprompt-core-agent = { git = "https://github.com/systempromptio/systemprompt-core", tag = "v0.0.1" }
```

Then in your service crate:

```toml
[dependencies]
systemprompt-models.workspace = true
systemprompt-core-api.workspace = true
```

### For Core Contributors

```bash
# Clone repository
git clone https://github.com/systempromptio/systemprompt-core
cd systemprompt-core

# Install web dependencies
cd web && npm install && cd ..

# Build all crates
cargo build --workspace

# Run tests (tests are in separate crates)
cargo test --manifest-path crates/tests/Cargo.toml

# Build specific crate
cargo build -p systemprompt-core-api

# Run CLI
cargo run --bin systemprompt -- --help
```

## Available Crates

### Shared Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-traits` | Core trait definitions |
| `systemprompt-models` | Data models and config types |
| `systemprompt-identifiers` | Typed ID generators |
| `systemprompt-client` | HTTP client |
| `systemprompt-extension` | Extension framework |

### Infrastructure Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-core-database` | SQLx abstraction (PostgreSQL) |
| `systemprompt-core-events` | Event bus, SSE infrastructure |
| `systemprompt-core-security` | JWT, auth utilities |
| `systemprompt-core-config` | Configuration loading |
| `systemprompt-core-logging` | Tracing setup |
| `systemprompt-cloud` | Cloud API, tenant management |

### Domain Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-core-users` | User management |
| `systemprompt-core-oauth` | OAuth2/OIDC authentication |
| `systemprompt-core-files` | File storage |
| `systemprompt-core-analytics` | Metrics and tracking |
| `systemprompt-core-content` | Content management |
| `systemprompt-core-ai` | LLM integration |
| `systemprompt-core-mcp` | MCP protocol implementation |
| `systemprompt-core-agent` | Agent orchestration (A2A) |

### Application Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-runtime` | AppContext, lifecycle management |
| `systemprompt-core-scheduler` | Job scheduling |
| `systemprompt-generator` | Static site generation |
| `systemprompt-sync` | Sync services |

### Entry Layer

| Crate | Description |
|-------|-------------|
| `systemprompt-core-api` | HTTP server framework |
| `systemprompt-cli` | Command-line interface |
| `systemprompt-core-tui` | Terminal UI |

## Extension Framework

Extensions enable downstream projects to extend core functionality without modifying it.

```rust
use systemprompt_extension::*;

struct MyExtension;
impl Extension for MyExtension { ... }
impl ApiExtension for MyExtension { ... }

register_extension!(MyExtension);
register_api_extension!(MyExtension);
```

**Available extension traits:**

| Trait | Purpose |
|-------|---------|
| `Extension` | Base trait - ID, name, version, dependencies |
| `SchemaExtension` | Database table definitions |
| `ApiExtension` | HTTP route handlers |
| `ConfigExtensionTyped` | Config validation at startup |
| `JobExtension` | Background job definitions |
| `ProviderExtension` | Custom LLM/tool provider implementations |

Extensions are discovered at runtime via the `inventory` crate.

## Module System

Domain crates include a `module.yaml` defining:

```yaml
name: users
version: "0.1.0"
display_name: "User Management"
type: core
weight: 1

schemas:
  - file: "migrations/001_users.sql"
    table: users
    required_columns: [id, email, created_at]

tables_created: [users, user_roles]

api:
  enabled: true
  path_prefix: "/api/v1/users"
```

## Code Quality

SystemPrompt enforces strict code quality through automated tooling:

```bash
# Format code
cargo fmt

# Run clippy with strict rules
cargo clippy --workspace
```

**Standards:**

- No `unsafe` code (forbidden)
- No `.unwrap()` (denied)
- Low cognitive complexity
- Pedantic clippy lints enabled
- All tests in separate test crates

## Testing

Tests are in separate crates under `crates/tests/` for faster incremental builds:

```bash
# Run all tests
cargo test --manifest-path crates/tests/Cargo.toml

# Run specific test crate
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-integration-tests
```

## Versioning

Follows [Semantic Versioning](https://semver.org/):

- **Major**: Breaking API changes
- **Minor**: New features, backward compatible
- **Patch**: Bug fixes, backward compatible

Current version: **0.0.1**

## License

FSL-1.1-ALv2 (Functional Source License) - see [LICENSE](LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/systempromptio/systemprompt-core)
- [Issues](https://github.com/systempromptio/systemprompt-core/issues)
- [Documentation](https://docs.systemprompt.io)
