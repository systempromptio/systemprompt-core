# SystemPrompt Blog

Services wrapper around the SystemPrompt core platform. The API runs from `core/`, custom services live in `crates/services/`.

## Rust Standards

**MANDATORY**: Follow `instructions/rust.md` for all Rust code. Key rules:
- Zero inline comments - code documents itself through naming
- Zero inline logging - use `LogService` exclusively
- Zero raw String IDs - use typed identifiers from `systemprompt_identifiers`
- Zero tech debt - refactor immediately when violations found, never defer

## Architecture

```
systemprompt-blog/
├── core/                           # Platform engine (git subtree)
│   └── crates/modules/             # api, agent, mcp, database, config, log, ai, oauth, users, blog, scheduler, news_fetcher, core
├── crates/services/
│   ├── mcp/                        # Rust MCP servers (workspace members)
│   │   ├── systemprompt-admin/     # Admin tools (port 5002)
│   │   ├── content-manager/        # Content management
│   │   └── tyingshoelaces/         # Blog-specific tools
│   ├── agents/                     # YAML config files (not Rust)
│   └── skills/                     # Config directories (12 skills)
└── infrastructure/configs/         # mcp-servers.yml, base.yml
```

## Rust Patterns

### Error Handling
```rust
use anyhow::{Context, Result};
use thiserror::Error;

// Standard return type
fn do_thing() -> Result<T> { ... }

// Add context
.context("Failed to initialize")?

// Custom errors with thiserror
#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Task UUID missing")]
    MissingTaskUuid,
    #[error("JSON parse error for '{field}': {source}")]
    JsonParse { field: String, #[source] source: serde_json::Error },
}
```

### Async
```rust
#[tokio::main]
async fn main() -> Result<()> { ... }

// Async traits
#[async_trait]
pub trait ServiceLifecycle: Send + Sync {
    async fn initialize(config: Config) -> Result<Self>;
    async fn start(&mut self) -> Result<()>;
}

// Logging: ignore errors with .ok()
logger.info("module", "message").await.ok();
```

### Module Structure
```
crate/
├── main.rs           # #[tokio::main] entry
├── lib.rs            # pub mod exports
├── server/           # Server struct (Clone + Arc<T> fields)
├── tools/            # Tool handlers
├── repository/       # Database operations
├── models/           # Domain types
└── resources/        # Prompts, resources
```

### Constructor Pattern
```rust
#[derive(Clone)]
pub struct Server {
    pub(super) db_pool: DbPool,
    pub(super) prompts: Arc<Prompts>,
}

impl Server {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool, prompts: Arc::new(Prompts::new()) }
    }
}
```

### Database

**CORE RULES (NON-NEGOTIABLE)**:
| Rule | Enforcement |
|------|-------------|
| Services rely on Repos | Services NEVER import `DatabaseQueryEnum` or execute queries directly |
| Repos NEVER have inline SQL | All SQL lives in `queries/` folder as `.sql` files |
| Queries exported via DB enums | Every query has a `DatabaseQueryEnum` variant that maps to its `.sql` file |

```
SERVICE → REPOSITORY → QUERY
(business logic)  (data access)   (SQL files)
```

```rust
// SERVICE (correct - calls repository, never touches DB directly)
let users = self.user_repository.list_users().await?;

// VIOLATION: Inline SQL in service
let query = "SELECT * FROM users WHERE id = $1";  // FORBIDDEN
```

**Repository Pattern with SQLX**:

Repositories accept `DbPool` (`Arc<Database>`) and use SQLX macros internally:

```rust
use systemprompt_core_database::DbPool;
use sqlx::PgPool;
use std::sync::Arc;

pub struct UserRepository {
    pool: Arc<PgPool>,
}

impl UserRepository {
    pub fn new(db: DbPool) -> Self {
        let pool = db.pool_arc().expect("Database must be PostgreSQL");
        Self { pool }
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
            .fetch_optional(&**self.pool)
            .await
    }
}
```

**Key Points**:
- `DbPool` = `Arc<Database>` (the abstraction layer)
- Extract `Arc<PgPool>` via `db.pool_arc()` in constructor
- Use `&**self.pool` to get `&PgPool` for SQLX macros

### Logging
```rust
// Always use LogService, variable name: logger
let logger = LogService::system(db_pool.clone());
logger.info("module_name", "message").await.ok();
```

## A2A Protocol

### Core Types
- **Message**: `role` (user/agent), `parts`, `messageId`, `contextId`
- **Task**: `id`, `contextId`, `status`, `history`, `artifacts`
- **TaskState**: Pending → Submitted → Working → Completed/Failed/Canceled
- **AgentCard**: Capabilities, skills (from MCP), security schemes
- **Artifact**: Generated outputs with fingerprints

### JSON-RPC Methods
- `message/send` - Send user message
- `message/stream` - Streaming variant
- `tasks/get` - Query task status
- `tasks/cancel` - Cancel execution
- `agent/getAuthenticatedExtendedCard` - Get capabilities

## MCP Protocol

### Orchestrator Pattern
```
McpOrchestrator
├── RegistryManager    # Load server manifests
├── LifecycleManager   # Start/stop/restart
├── DatabaseManager    # State persistence
├── MonitoringManager  # Health checks
└── EventBus           # Async events
```

### Events
`ServiceStarted`, `ServiceFailed`, `ServiceStopped`, `HealthCheckFailed`, `SchemaUpdated`

Tool results transform to A2A Artifacts via `ToolResultHandler`.

## SSE Broadcasting Pattern

**CRITICAL**: Agent services run as separate processes from the API. `Lazy` statics like `CONTEXT_BROADCASTER` are process-local - the agent's broadcaster has 0 SSE connections.

| Scenario | Solution |
|----------|----------|
| API process broadcasting | Use `CONTEXT_BROADCASTER` directly |
| Agent/worker broadcasting | Send HTTP POST to webhook → API broadcasts |

**Working**: `execution_step` → HTTP POST to `/api/v1/contexts/webhook` → API broadcasts → ✓
**Broken**: `task_created` → direct `CONTEXT_BROADCASTER` call → 0 connections → ✗

**Code-Level Enforcement**: Services use `Arc<dyn BroadcastClient>` instead of `CONTEXT_BROADCASTER` directly. Create via `create_webhook_broadcaster(token)` for agent/MCP code.

See `instructions/rust.md` § 6 for full pattern with code examples.

## Quick Reference

### Commands
```bash
cd core && just status          # Service status
cd core && just logs            # Stream logs
cargo build --workspace         # Build all services
cargo build -p systemprompt-admin  # Build specific service
```

### Ports
| Service | Port |
|---------|------|
| API Server | 8080 |
| Admin MCP | 5002 |

### Key Paths
| Path | Purpose |
|------|---------|
| `instructions/rust.md` | **Rust coding standards (MANDATORY)** |
| `instructions/logs.md` | Log querying documentation |
| `core/src/main.rs` | CLI entry point |
| `core/target/debug/systemprompt` | Built CLI |
| `crates/services/mcp/*/` | MCP server implementations |
| `crates/services/agents/*.yml` | Agent configs |
| `infrastructure/configs/` | Service configs |

### Config Files
- `.env` - Environment variables
- `infrastructure/configs/mcp-servers.yml` - MCP server definitions
- `infrastructure/configs/base.yml` - Base system config

### Database Connections
| Environment | Connection |
|-------------|------------|
| Local | `postgresql://systemprompt:systemprompt_dev_password@localhost:5432/systemprompt_dev` |
| Production | `postgresql://blog_user:blogpass123@database.systemprompt.io:6432/site_blog` |

Switch environments: `just use-local` / `just use-prod`

## Log Querying

See `instructions/logs.md` for detailed documentation. Quick examples:

```bash
# Query recent errors
just query "SELECT timestamp, level, module, message FROM logs WHERE level = 'ERROR' ORDER BY timestamp DESC LIMIT 20"

# Filter by task ID (use partial match with LIKE)
just query "SELECT timestamp, level, module, message, context_id FROM logs WHERE task_id LIKE '%08a90adb%' ORDER BY timestamp DESC"

# Filter by context ID
just query "SELECT timestamp, module, message FROM logs WHERE context_id = 'your-context-id' ORDER BY timestamp"

# Output as JSON
just query "SELECT * FROM logs LIMIT 5" json
```

Logs table columns: `id`, `timestamp`, `level`, `module`, `message`, `metadata`, `user_id`, `session_id`, `task_id`, `trace_id`, `context_id`, `client_id`
