# SystemPrompt Core

The core platform engine for SystemPrompt - a multi-tenant AI agent platform with A2A protocol support, MCP integration, and cloud deployment.

## Architecture

```
systemprompt-core/
├── crates/
│   ├── shared/           # Foundation layer (no dependencies on other layers)
│   │   ├── models/       # systemprompt-models - Core data types
│   │   ├── traits/       # systemprompt-traits - Core interfaces
│   │   ├── identifiers/  # systemprompt-identifiers - Typed IDs
│   │   ├── extension/    # systemprompt-extension - Extension framework
│   │   ├── provider-contracts/  # Provider trait definitions
│   │   ├── client/       # HTTP API client
│   │   └── template-provider/   # Template traits
│   │
│   ├── infra/            # Infrastructure layer
│   │   ├── database/     # systemprompt-database - SQLx abstraction
│   │   ├── events/       # systemprompt-events - Event bus, SSE
│   │   ├── security/     # systemprompt-security - JWT, auth
│   │   ├── config/       # systemprompt-config - Config loading
│   │   ├── logging/      # systemprompt-logging - Tracing setup
│   │   ├── loader/       # systemprompt-loader - File/module discovery
│   │   └── cloud/        # systemprompt-cloud - Cloud API, tenants
│   │
│   ├── domain/           # Business logic layer
│   │   ├── users/        # systemprompt-users - User management
│   │   ├── oauth/        # systemprompt-oauth - OAuth2/OIDC
│   │   ├── files/        # systemprompt-files - File storage
│   │   ├── analytics/    # systemprompt-analytics - Metrics
│   │   ├── content/      # systemprompt-content - Content management
│   │   ├── ai/           # systemprompt-ai - LLM integration
│   │   ├── mcp/          # systemprompt-mcp - MCP protocol
│   │   ├── agent/        # systemprompt-agent - A2A protocol
│   │   └── templates/    # systemprompt-templates - Template registry
│   │
│   ├── app/              # Application services layer
│   │   ├── runtime/      # systemprompt-runtime - AppContext, lifecycle
│   │   ├── scheduler/    # systemprompt-scheduler - Job scheduling
│   │   ├── generator/    # systemprompt-generator - Static site gen
│   │   └── sync/         # systemprompt-sync - Cloud sync
│   │
│   ├── entry/            # Application boundaries
│   │   ├── api/          # systemprompt-api - HTTP server
│   │   └── cli/          # systemprompt-cli - CLI application
│   │
│   └── tests/            # Separate test workspace (excluded from main)
│
├── systemprompt/         # Facade crate - re-exports with feature flags
├── defaults/             # Default templates, assets, web content
└── instructions/         # Documentation
    └── information/      # Architecture, boundaries, config docs
```

## Dependency Flow

```
Entry (api, cli) → App (runtime, scheduler) → Domain (agent, ai, mcp...) → Infra (database, events...) → Shared (models, traits)
```

**Rule**: Dependencies flow downward only. No circular dependencies.

## Key Documentation

| Document | Purpose |
|----------|---------|
| `instructions/information/architecture.md` | Full crate taxonomy, extension framework, paths |
| `instructions/information/boundaries.md` | Module boundary rules, acceptable patterns |
| `instructions/information/config.md` | Configuration system (profiles, secrets, credentials) |
| `instructions/information/cloud.md` | Cloud deployment and tenant management |
| `instructions/rust.md` | Rust coding standards |

## Rust Standards

**MANDATORY**: Follow `instructions/rust.md`. Key rules:
- Zero inline comments - code documents itself through naming
- Zero raw String IDs - use typed identifiers from `systemprompt_identifiers`
- Services call repositories, never execute SQL directly
- All SQL in `.sql` files, never inline

## Facade Crate (`systemprompt/`)

Re-exports all functionality with feature flags:

| Feature | Includes |
|---------|----------|
| `core` (default) | traits, models, identifiers, extension |
| `database` | Database abstraction |
| `api` | HTTP server, AppContext (requires core + database) |
| `cli` | CLI entry point |
| `full` | Everything: all domain modules + CLI |

```rust
// Using the facade
use systemprompt::prelude::*;
use systemprompt::database::DbPool;
```

## Extension Framework

Extensions use the `inventory` crate for compile-time registration:

```rust
use systemprompt::extension::prelude::*;

struct MyExtension;
impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata { ... }
    fn schemas(&self) -> Vec<SchemaDefinition> { ... }
    fn router(&self) -> Option<ExtensionRouter> { ... }
}

register_extension!(MyExtension);
```

**Key traits**: `Extension`, `SchemaExtensionTyped`, `ApiExtensionTyped`, `JobExtensionTyped`, `ProviderExtensionTyped`

## Configuration

Profiles are the single source of truth - no environment variable fallbacks:

```yaml
# .systemprompt/profiles/local/profile.yaml
name: local
database:
  type: postgres
  url: postgresql://user:pass@localhost:5432/db
server:
  host: 127.0.0.1
  port: 8080
paths:
  system: /var/www/html/myapp
  services: /var/www/html/myapp/services
secrets:
  secrets_path: ../secrets/local.secrets.json
```

**Bootstrap sequence**: ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config → AppContext

## CLI Commands

```bash
# Services
systemprompt infra services start --all
systemprompt infra services status
systemprompt infra services stop --all

# Database
systemprompt infra db status
systemprompt infra db migrate
systemprompt infra db query "SELECT * FROM users LIMIT 10"

# Agents
systemprompt admin agents list
systemprompt admin agents status my-agent

# Cloud
systemprompt cloud auth login
systemprompt cloud tenant create
systemprompt cloud deploy
```

## Database Pattern

```rust
use systemprompt_database::DbPool;

pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: &UserId) -> Result<Option<User>> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id.as_str())
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(Into::into)
    }
}
```

## A2A Protocol

Core types in `crates/domain/agent/`:
- **Message**: `role`, `parts`, `messageId`, `contextId`
- **Task**: `id`, `contextId`, `status`, `history`, `artifacts`
- **TaskState**: Pending → Submitted → Working → Completed/Failed/Canceled

## MCP Protocol

MCP implementation in `crates/domain/mcp/`:
- Server lifecycle management
- Tool/resource discovery
- Transport protocols (stdio, SSE)

## Key Files

| File | Purpose |
|------|---------|
| `crates/entry/cli/src/main.rs` | CLI entry point |
| `crates/entry/api/src/main.rs` | API server entry |
| `crates/app/runtime/src/context.rs` | AppContext definition |
| `crates/shared/models/src/config.rs` | Config struct |
| `crates/shared/extension/src/lib.rs` | Extension trait |

## Testing

Tests are in a separate workspace at `crates/tests/` (excluded from main workspace):

```bash
# Run all tests
cargo test --manifest-path crates/tests/Cargo.toml --workspace

# Run specific test crate
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-agent-tests
```

## Building

```bash
# Debug build
cargo build --workspace

# Release build
cargo build --release --workspace

# Specific crate
cargo build -p systemprompt-cli
```
