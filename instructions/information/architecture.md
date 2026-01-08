# Crate Architecture

This document defines the crate taxonomy for systemprompt-core. Every crate belongs to exactly one layer with strict criteria.

---

## Crate Layers

```
crates/
  shared/     # Pure types, zero internal dependencies
  infra/      # Stateless infrastructure utilities
  domain/     # Bounded contexts with SQL + repos + services
  app/        # Orchestration, no business logic
  entry/      # Entry points (binaries, public APIs)

systemprompt/   # Facade: Public API for external consumers (crates.io)
```

---

## Layer Definitions

### Shared Layer (`crates/shared/`)

Pure types with zero dependencies on other systemprompt crates.

| Criterion | Rule |
|-----------|------|
| SQL/Database | NEVER |
| Repository | NEVER |
| Service | NEVER |
| Internal deps | NEVER (external crates only) |
| State | NEVER (no singletons, no mutability) |
| I/O | NEVER (no file, network, database) |

**Allowed contents**: Type definitions, trait definitions, constants, pure functions, derive macros, type aliases.

| Crate | Purpose |
|-------|---------|
| `identifiers/` | Typed IDs (`UserId`, `TaskId`, etc.) |
| `models/` | Domain models, API types, configuration structs, **validation report types** |
| `traits/` | Shared trait definitions (`LlmProvider`, `ToolProvider`, `Job`, **`DomainConfig`**) |
| `client/` | HTTP client for external API access |
| `extension/` | Extension framework for user customization |

---

### Infrastructure Layer (`crates/infra/`)

Stateless utilities providing cross-cutting concerns. May have I/O but no persistent domain state.

| Criterion | Rule |
|-----------|------|
| SQL/Database | Only `database/` crate (provides abstraction) |
| Repository | NEVER (no domain-specific repos) |
| Service | Stateless only (no business logic) |
| Business logic | NEVER |
| Singletons | Allowed for global resources |
| Can depend on | `shared/` only |

**Test**: If removing all database calls leaves the crate fully functional, it belongs in `infra/`. If it breaks, it belongs in `domain/`.

| Crate | Purpose |
|-------|---------|
| `database/` | SQLx abstraction, connection pooling, base repository trait |
| `events/` | Event bus, broadcasters, SSE infrastructure |
| `security/` | JWT validation, token extraction, cookie handling |
| `config/` | Configuration loading, environment handling |
| `logging/` | Tracing setup, log sinks, database layer |
| `cloud/` | Cloud API client, tenant management, checkout flow, credentials |

---

### Domain Layer (`crates/domain/`)

Full bounded contexts. Each crate owns its database tables, repositories, and services.

| Criterion | Rule |
|-----------|------|
| SQL/Database | YES (required) |
| Repository | YES (required, in `src/repository/`) |
| Service | YES (required, in `src/services/`) |
| Module definition | YES (in `crates/infra/loader/src/modules/`) |
| Bounded context | YES (single domain responsibility) |
| Can depend on | `shared/`, `infra/` |
| Cross-domain deps | NEVER (use traits or events) |

**Required structure**:

```
domain/{name}/
  Cargo.toml
  schema/             # SQL schema files
    {table}.sql
  src/
    lib.rs            # Public API
    error.rs          # Domain-specific errors
    models/           # Domain models (or re-export from shared)
    repository/       # Data access layer
      mod.rs
      {entity}_repository.rs
    services/         # Business logic
      mod.rs
      {entity}_service.rs
```

| Crate | Bounded Context | Key Entities |
|-------|-----------------|--------------|
| `users/` | User identity | User, Role |
| `oauth/` | Authentication | Token, Client, Grant, Session |
| `files/` | File storage | File, FileMetadata |
| `analytics/` | Metrics & tracking | Session, Event, Metric |
| `content/` | Content management | Content, Category, Tag |
| `ai/` | LLM integration | Request, Response, Provider |
| `mcp/` | MCP protocol | Server, Tool, Deployment |
| `agent/` | A2A protocol | Agent, Task, Context, Skill |

---

### Application Layer (`crates/app/`)

Orchestration without business logic. Coordinates domain crates for workflows.

| Criterion | Rule |
|-----------|------|
| SQL/Database | Optional (job tracking only) |
| Business logic | NEVER (delegates to domain) |
| Can depend on | `shared/`, `infra/`, `domain/` |
| Purpose | Workflows, job scheduling, pipelines |

| Crate | Purpose |
|-------|---------|
| `scheduler/` | Job scheduling, cron execution |
| `generator/` | Static site generation |
| `runtime/` | **StartupValidator**, AppContext, lifecycle management |

---

### Entry Layer (`crates/entry/`)

Entry points that wire the application together.

| Criterion | Rule |
|-----------|------|
| Entry point | YES (`main.rs` or public library API) |
| Business logic | NEVER (pure wiring) |
| Can depend on | All layers |

| Crate | Purpose |
|-------|---------|
| `cli/` | Command-line interface |
| `api/` | HTTP gateway, route handlers, middleware |
| `tui/` | Terminal UI |

---

### Facade Layer (`systemprompt/`)

Public API for external consumers. Published to crates.io for downstream projects. Located at root level (not in crates/) for cleaner import paths.

| Criterion | Rule |
|-----------|------|
| Re-exports | YES (exposes internal crates via modules) |
| New code | NEVER (only re-exports and feature flags) |
| Feature flags | YES (granular opt-in for functionality) |
| Can depend on | All layers |

| Crate | Purpose |
|-------|---------|
| `systemprompt/` | Unified facade with feature-gated re-exports |

**Feature flags:**

| Feature | Includes |
|---------|----------|
| `core` (default) | traits, models, identifiers, extension |
| `database` | database abstraction |
| `api` | HTTP server, AppContext |
| `cli` | CLI entry point (`systemprompt::cli::run()`) |
| `full` | Everything: all domain modules + CLI |

---

### Extension Framework (`crates/shared/extension/`)

The extension system enables downstream projects to extend core functionality without modifying it.

**Extension traits:**

| Trait | Purpose |
|-------|---------|
| `Extension` | Base trait - ID, name, version, dependencies |
| `SchemaExtension` | Database table definitions |
| `ApiExtension` | HTTP route handlers |
| `ConfigExtensionTyped` | Config validation - validated at startup by `StartupValidator` |
| `JobExtension` | Background job definitions |
| `ProviderExtension` | Custom LLM/tool provider implementations |

**Discovery mechanism:**

Extensions use the `inventory` crate for compile-time registration:

```rust
use systemprompt_extension::*;

struct MyExtension;
impl Extension for MyExtension { ... }
impl ApiExtension for MyExtension { ... }

register_extension!(MyExtension);
register_api_extension!(MyExtension);
```

At runtime, `ExtensionRegistry::discover()` collects all registered extensions.

---

### Storage Path Constants

Storage paths are centralized in `infra/cloud/src/constants.rs` to ensure consistency across core and extensions.

**Core storage structure:**

```
storage/                          <- profile.paths.storage
  files/                          <- storage::FILES
    images/                       <- storage::IMAGES
      generated/                  <- storage::GENERATED
      logos/                      <- storage::LOGOS
      {extension}/                <- Extension-specific (e.g., blog/, social/)
    audio/                        <- storage::AUDIO
    video/                        <- storage::VIDEO
    documents/                    <- storage::DOCUMENTS
    uploads/                      <- storage::UPLOADS
```

**Using paths in core:**

```rust
use systemprompt_cloud::constants::storage;

let images_path = storage_root.join(storage::IMAGES);      // storage/files/images
let generated = storage_root.join(storage::GENERATED);     // storage/files/images/generated
let audio = storage_root.join(storage::AUDIO);             // storage/files/audio
```

**Using paths in extensions:**

Extensions declare required storage paths via `required_storage_paths()`. These are:
1. Included in generated Dockerfiles (mkdir commands)
2. Available for validation via `ConfigExtensionTyped`

```rust
use systemprompt_extension::Extension;

impl Extension for BlogExtension {
    fn required_storage_paths(&self) -> Vec<&'static str> {
        vec!["files/images/blog"]
    }
}
```

**Profile configuration:**

The `paths.storage` in profile.yaml points to the **root** storage directory:

```yaml
paths:
  storage: /var/www/html/myproject/storage  # Root, NOT storage/files
```

**Key rules:**

| Rule | Description |
|------|-------------|
| Core owns structure | Core defines `files/`, `images/`, `audio/`, etc. |
| Extensions own subdirs | Extensions define paths like `files/images/blog/` |
| Profile points to root | `paths.storage` = root storage dir (not `storage/files`) |
| Use constants | Always use `storage::*` constants, never hardcode paths |
| Dockerfile discovery | Extensions register paths via `required_storage_paths()` |

---

### Product Binary Pattern

Template/product repositories must own the final binary to include extension jobs. Core provides reusable entry points; products compose them with extensions.

**Architecture:**

```
┌──────────────────────────────────────────────────────────┐
│  Product Repository (template)                           │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │  src/lib.rs (FACADE)                               │ │
│  │  - Re-exports core: pub use systemprompt::*        │ │
│  │  - Exports extensions: pub use blog_extension as   │ │
│  └────────────────────────────────────────────────────┘ │
│                          │                               │
│  ┌────────────────────────────────────────────────────┐ │
│  │  src/main.rs (BINARY)                              │ │
│  │  - Uses facade (forces all linkage)                │ │
│  │  - Delegates to systemprompt_cli::run()            │ │
│  └────────────────────────────────────────────────────┘ │
│           │                              │               │
│           ▼                              ▼               │
│  ┌─────────────────┐          ┌─────────────────────┐   │
│  │ core/           │          │ extensions/         │   │
│  │ (submodule)     │          │ └── blog/           │   │
│  │ - systemprompt  │          │     └── jobs/       │   │
│  │ - CLI run()     │          │                     │   │
│  └─────────────────┘          └─────────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

**Why this pattern:**

The `inventory` crate uses static initialization. `submit_job!()` registers jobs in a static collector, but statics are only included if the crate is linked into the binary.

Core's CLI binary only links core crates. To include extension jobs, the product must own the binary that links both core and extensions.

**Product structure:**

| File | Purpose |
|------|---------|
| `src/lib.rs` | Facade re-exporting core + extensions |
| `src/main.rs` | Binary calling `systemprompt_cli::run()` |
| `Cargo.toml` | `[[bin]]` target + all dependencies |

**Example product Cargo.toml:**

```toml
[package]
name = "my-product"

[lib]
path = "src/lib.rs"

[[bin]]
name = "systemprompt"
path = "src/main.rs"

[dependencies]
systemprompt = { path = "core/systemprompt" }
systemprompt-cli = { path = "core/crates/entry/cli" }
my-blog-extension = { path = "extensions/blog" }
```

**Example product main.rs:**

```rust
use my_product as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    systemprompt_cli::run().await
}
```

The `use my_product as _;` forces the facade (and all its dependencies) to be linked, pulling in extension job registrations.

**Migration weights:**

Schema extensions define `migration_weight()` to control installation order:
- Core modules: 1-10
- User extensions: 100+

This ensures core tables exist before extension tables that reference them.

---

### Module System

Modules are defined in Rust code at `crates/infra/loader/src/modules/`. Each module file uses `include_str!()` to embed SQL schemas at compile time.

**Structure:**

```
crates/infra/loader/src/modules/
  mod.rs           # pub fn all() -> Vec<Module>
  database.rs      # pub fn define() -> Module
  log.rs
  users.rs
  oauth.rs
  mcp.rs
  files.rs
  content.rs
  ai.rs
  analytics.rs
  agent.rs
  scheduler.rs
  api.rs
```

**Module definition pattern:**

```rust
// crates/infra/loader/src/modules/users.rs
use systemprompt_models::modules::{Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        name: "users".into(),
        weight: Some(1),
        schemas: Some(vec![
            ModuleSchema {
                table: "users".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/users.sql").into()
                ),
                required_columns: vec!["id".into(), "email".into()],
            },
        ]),
        // ...
    }
}
```

**Benefits:**
- Compile-time SQL validation (missing file = compile error)
- No YAML parsing at runtime
- Matches extension pattern (`SchemaSource::Inline`)
- Works in Docker without source tree

**Adding a new module:**
1. Create SQL files in `domain/{name}/schema/`
2. Create `modules/{name}.rs` with `pub fn define() -> Module`
3. Add `mod {name};` and call in `modules/mod.rs`

Extensions use a separate discovery mechanism via the `inventory` crate and `ExtensionRegistry::discover()`. See [Extension Framework](#extension-framework-cratessharedextension) above.

---

### Subprocess Config/Secrets Propagation

When spawning subprocesses (agents, MCP servers), config and secrets must be passed explicitly. **No fuzzy profile discovery in subprocesses.**

**Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│ API Server (Parent Process)                                  │
│ - Loads profile from: /path/to/profile.yaml                 │
│ - Loads secrets from: /path/to/secrets.json                 │
│ - JWT_SECRET = "abc123..."                                  │
└──────────────────────┬──────────────────────────────────────┘
                       │
          ┌────────────┴────────────┐
          │ Spawns subprocesses     │
          │ with explicit env vars: │
          │ - SYSTEMPROMPT_PROFILE  │
          │ - JWT_SECRET            │
          │ - DATABASE_URL          │
          └────────────┬────────────┘
                       │
       ┌───────────────┼───────────────┐
       ▼               ▼               ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ Agent :9000  │ │ Agent :9001  │ │ MCP :9100    │
│ (from env)   │ │ (from env)   │ │ (from env)   │
└──────────────┘ └──────────────┘ └──────────────┘
```

**Key files:**
- `crates/domain/agent/src/services/agent_orchestration/process.rs` - Agent spawning
- `crates/domain/mcp/src/services/process/spawner.rs` - MCP spawning
- `crates/shared/models/src/secrets.rs` - Secrets loading (env var priority)

**Rules:**
- Parent process MUST pass `SYSTEMPROMPT_PROFILE` and `JWT_SECRET` to all subprocesses
- Subprocesses MUST prioritize env vars over file discovery
- Never rely on fuzzy profile discovery in subprocesses
- JWT secrets must be identical across all processes for token validation

---

### Config Validation System

The startup validation system ensures configuration is valid before the application runs.

**Architecture:**

```
┌─────────────────────────────────────────┐
│        Extensions (Blog, etc.)          │  ← Register via inventory
└────────────────────┬────────────────────┘
                     │ ConfigExtensionTyped
┌────────────────────▼────────────────────┐
│   APP Layer (StartupValidator)          │  ← Orchestrates all validation
└────────────────────┬────────────────────┘
                     │
┌────────────────────▼────────────────────┐
│   DOMAIN Layer (domain validators)      │  ← DomainConfig implementations
└────────────────────┬────────────────────┘
                     │
┌────────────────────▼────────────────────┐
│  INFRA Layer (schema validation)        │  ← YAML parsing, schema checks
└────────────────────┬────────────────────┘
                     │
┌────────────────────▼────────────────────┐
│ SHARED Layer (traits, types)            │  ← DomainConfig trait, ValidationReport
└─────────────────────────────────────────┘
```

**Key components:**

| Component | Location | Purpose |
|-----------|----------|---------|
| `DomainConfig` trait | `shared/traits/` | Interface for domain validators |
| `ValidationReport` | `shared/models/` | Unified validation result types |
| `StartupValidator` | `app/runtime/` | Orchestrates domain + extension validation |
| Domain validators | `domain/*/config/` | Domain-specific semantic validation |

**Startup sequence:**

1. `ProfileBootstrap::init()` - Load profile YAML
2. `Config::from_profile()` - Build config, validate paths exist
3. `StartupValidator::validate()` - Run all domain and extension validators
4. If errors → display report → `exit(1)` (no bypass)
5. If warnings → display → continue
6. Execute command

**Key rules:**

- Config profiles are **required** - no env var fallbacks
- Path validation happens at **startup**, not command execution
- All domains **must** implement `DomainConfig` trait
- Startup validation is **always blocking** - no `--force` bypass
- Core **never** references extensions - they register via `inventory`

---

## Dependency Rules

### Flow Diagram

```
┌─────────────────────────────────────────┐
│        FACADE (systemprompt)            │  ◄── External consumers (crates.io)
└────────────────────┬────────────────────┘
                     │ re-exports
┌────────────────────▼────────────────────┐
│            ENTRY (api, tui)             │
└────────────────────┬────────────────────┘
                     │ depends on
┌────────────────────▼────────────────────┐
│   APP (runtime, scheduler, generator)   │
└────────────────────┬────────────────────┘
                     │ depends on
┌────────────────────▼────────────────────┐
│  DOMAIN (users, oauth, ai, agent, ...)  │
└────────────────────┬────────────────────┘
                     │ depends on
┌────────────────────▼────────────────────┐
│ INFRA (database, events, security, ...) │
└────────────────────┬────────────────────┘
                     │ depends on
┌────────────────────▼────────────────────┐
│ SHARED (models, traits, identifiers,    │
│         extension)                      │
└─────────────────────────────────────────┘
```

### Extension Integration

```
┌─────────────────────────────────────────────────────────────┐
│                    User Project (template)                   │
│                                                              │
│  ┌──────────────────┐    ┌──────────────────────────────┐  │
│  │  Custom          │    │  register_extension!()       │  │
│  │  Extensions      │───►│  register_api_extension!()   │  │
│  │                  │    │  register_schema_extension!()│  │
│  └──────────────────┘    └──────────────────────────────┘  │
│                                      │                       │
└──────────────────────────────────────│───────────────────────┘
                                       │ inventory collects
                                       ▼
┌─────────────────────────────────────────────────────────────┐
│                    Core (systemprompt-core)                  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  ExtensionRegistry::discover()                        │  │
│  │    ├── config_extensions() → StartupValidator         │  │
│  │    ├── schema_extensions() → install_extension_schemas│  │
│  │    ├── api_extensions() → mount_extension_routes      │  │
│  │    ├── job_extensions() → scheduler                   │  │
│  │    └── provider_extensions() → LLM/Tool providers     │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Forbidden Dependencies

| Layer | Cannot Depend On |
|-------|------------------|
| Shared | Any systemprompt crate (except within shared/) |
| Infra | domain/, app/, entry/, facade/ |
| Domain | Other domain crates, app/, entry/, facade/ |
| App | entry/, facade/ |
| Entry | facade/ |
| Facade | (no restrictions - can re-export anything) |

**Note:** The `extension` crate in `shared/` is special - it can depend on `shared/traits` to reference provider types like `LlmProvider` and `ToolProvider`.

---

## Cross-Domain Communication

Domain crates cannot depend on each other. Use these patterns:

### Pattern 1: Traits in Shared (Preferred)

Define abstraction in `shared/traits/`, implement in domain crate:

```rust
// shared/traits/src/context_provider.rs
pub trait ContextProvider: Send + Sync {
    async fn get_context(&self, id: &ContextId) -> Result<Context>;
}

// domain/agent/src/services/context_service.rs
impl ContextProvider for ContextService { ... }

// domain/ai/src/services/ai_service.rs
pub struct AiService {
    context_provider: Arc<dyn ContextProvider>,
}
```

### Pattern 2: Event-Driven

Publish events via `infra/events/`, subscribe in consuming crate:

```rust
// domain/agent/src/services/task_service.rs
self.event_bus.publish(TaskCompletedEvent { ... }).await;

// Subscriber in domain/ai listens via event bus
```

---

## Naming Conventions

### Crate Names

Remove `core` prefix: `systemprompt-core-ai` becomes `systemprompt-ai`.

| Layer | Pattern | Example |
|-------|---------|---------|
| Shared | `systemprompt-{name}` | `systemprompt-models` |
| Infra | `systemprompt-{name}` | `systemprompt-events` |
| Domain | `systemprompt-{domain}` | `systemprompt-users` |
| App | `systemprompt-{function}` | `systemprompt-scheduler` |
| Entry | `systemprompt-{entry}` | `systemprompt-api` |

---

## Testing Policy

All tests MUST be in separate test crates, never inline in source files.

### Test Crate Structure

```
crates/
  shared/
    extension/           # Source crate
      src/
        lib.rs           # NO #[cfg(test)] modules
      Cargo.toml
    extension-tests/     # Test crate
      src/
        lib.rs
        builder_tests.rs
        hlist_tests.rs
        registry_tests.rs
        types_tests.rs
      tests/
        compile_fail/    # trybuild compile-fail tests
      Cargo.toml
```

### Rules

| Rule | Description |
|------|-------------|
| No inline tests | Never use `#[cfg(test)] mod tests` in source files |
| Separate crate | Create `{crate-name}-tests` crate for each crate needing tests |
| Integration tests | Place in `tests/` directory of test crate |
| Compile-fail tests | Use trybuild in `tests/compile_fail/` |
| Dependencies | Test crate depends on source crate as dev-dependency |

### Benefits

1. **Faster incremental builds** - Source crates don't recompile when tests change
2. **Cleaner separation** - Source code isn't polluted with test fixtures
3. **Better IDE performance** - Less code to analyze in source files
4. **Explicit dependencies** - Test-only dependencies stay in test crate

### Test Crate Cargo.toml Template

```toml
[package]
name = "systemprompt-{name}-tests"
version.workspace = true
edition.workspace = true
publish = false  # Test crates are never published

[dependencies]
systemprompt-{name} = { path = "../{name}" }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros"] }
trybuild = "1.0"
```

---

## Validation

Run these checks after adding or moving crates:

| Layer | Check | Command |
|-------|-------|---------|
| Shared | No internal deps | `grep "systemprompt-" crates/shared/*/Cargo.toml` → only shared crates |
| Shared | No SQL | `grep "sqlx" crates/shared/*/Cargo.toml` → empty |
| Infra | No domain deps | `grep "systemprompt-" crates/infra/*/Cargo.toml` → only shared/infra |
| Domain | Has repository | `ls crates/domain/*/src/repository/` → exists |
| Domain | Has services | `ls crates/domain/*/src/services/` → exists |
| Domain | No cross-domain | `grep "systemprompt-" crates/domain/*/Cargo.toml` → no other domain crates |

---

## Current Crate Inventory

### Shared Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `shared/traits` | `systemprompt-traits` | Core trait definitions |
| `shared/models` | `systemprompt-models` | Data models, config types |
| `shared/identifiers` | `systemprompt-identifiers` | Typed IDs |
| `shared/client` | `systemprompt-client` | HTTP client |
| `shared/extension` | `systemprompt-extension` | Extension framework |

### Infrastructure Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `infra/database` | `systemprompt-core-database` | SQLx abstraction |
| `infra/events` | `systemprompt-core-events` | Event bus, SSE |
| `infra/security` | `systemprompt-core-security` | JWT, auth utils |
| `infra/config` | `systemprompt-core-config` | Config loading |
| `infra/logging` | `systemprompt-core-logging` | Tracing setup |
| `infra/cloud` | `systemprompt-cloud` | Cloud API, tenants |

### Domain Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `domain/users` | `systemprompt-core-users` | User management |
| `domain/oauth` | `systemprompt-core-oauth` | OAuth2/OIDC |
| `domain/files` | `systemprompt-core-files` | File storage |
| `domain/analytics` | `systemprompt-core-analytics` | Metrics |
| `domain/content` | `systemprompt-core-content` | Content management |
| `domain/ai` | `systemprompt-core-ai` | LLM integration |
| `domain/mcp` | `systemprompt-core-mcp` | MCP protocol |
| `domain/agent` | `systemprompt-core-agent` | A2A protocol |

### Application Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `app/scheduler` | `systemprompt-core-scheduler` | Job scheduling |
| `app/generator` | `systemprompt-generator` | Static site gen |
| `app/sync` | `systemprompt-sync` | Sync services |
| `app/runtime` | `systemprompt-runtime` | AppContext, lifecycle |

### Entry Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `entry/cli` | `systemprompt-cli` | Command-line interface |
| `entry/api` | `systemprompt-core-api` | HTTP server |
| `entry/tui` | `systemprompt-core-tui` | Terminal UI |

### Facade Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `systemprompt/` (root) | `systemprompt` | Public API for crates.io |

