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

Pure types with zero dependencies on other systemprompt crates (except within shared/).

| Criterion | Rule |
|-----------|------|
| SQL/Database | NEVER |
| Repository | NEVER |
| Service | NEVER |
| Internal deps | Other shared/ crates only |
| State | NEVER (no singletons, no mutability) |
| I/O | NEVER (no file, network, database) |

**Allowed contents**: Type definitions, trait definitions, constants, pure functions, derive macros, type aliases.

| Crate | Purpose |
|-------|---------|
| `provider-contracts/` | Provider trait contracts (`LlmProvider`, `ToolProvider`, `Job`, `ComponentRenderer`, etc.) |
| `identifiers/` | Typed IDs (`UserId`, `TaskId`, etc.) |
| `models/` | Domain models, API types, configuration structs, **validation report types** |
| `traits/` | Infrastructure trait definitions (`DomainConfig`, `ConfigProvider`, `DatabaseHandle`) - re-exports provider-contracts |
| `template-provider/` | Template loading and rendering abstractions - re-exports provider-contracts |
| `client/` | HTTP client for external API access |
| `extension/` | Extension framework for user customization - depends on provider-contracts for provider traits |

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
| Extension | YES (in `src/extension.rs`, registers schemas via `register_extension!`) |
| Bounded context | YES (single domain responsibility) |
| Can depend on | `shared/`, `infra/` |
| Cross-domain deps | NEVER (use traits or events) |

**Required structure**:

```
domain/{name}/
  Cargo.toml
  schema/             # SQL schema files (no migrations/ subfolder)
    {table}.sql
  src/
    lib.rs            # Public API, exports extension
    extension.rs      # Extension trait implementation with schema registration
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

| Feature | Includes | Notes |
|---------|----------|-------|
| `core` (default) | traits, models, identifiers, extension, template-provider | Base types |
| `database` | database abstraction, sqlx | Data access |
| `mcp` | Model Context Protocol (rmcp) | MCP support |
| `api` | HTTP server, AppContext | Requires core + database |
| `sync` | Cloud synchronization services | |
| `cloud` | Cloud infrastructure, credentials, OAuth | |
| `test-utils` | Test utilities | Requires cloud |
| `cli` | CLI entry point (`systemprompt::cli::run()`) | |
| `runtime` | RuntimeBuilder for library embedding | Requires cli |
| `full` | All domain modules + infrastructure | Superset of all |

---

### Extension Framework (`crates/shared/extension/`)

The extension system enables downstream projects to extend core functionality without modifying it.

**Core trait:**

`Extension` - Base trait with 20+ optional capabilities: metadata, schemas, routes, jobs, config validation, providers, storage paths, RBAC roles, assets.

**Typed extension traits:**

| Trait | Purpose |
|-------|---------|
| `ExtensionType` | Compile-time constants: ID, NAME, VERSION, PRIORITY |
| `SchemaExtensionTyped` | Database table definitions with migration weights |
| `ApiExtensionTyped` | HTTP route handlers with base path and auth requirements |
| `JobExtensionTyped` | Background job definitions |
| `ProviderExtensionTyped` | Custom LLM/tool provider implementations |
| `ConfigExtensionTyped` | Config validation at startup |

**Capability traits:**

| Trait | Purpose |
|-------|---------|
| `HasConfig` | Access to configuration provider |
| `HasDatabase` | Access to database handle |
| `HasExtension<E>` | Access to another extension |
| `HasHttpClient` | Access to HTTP client |
| `HasEventBus` | Access to event publisher |
| `FullContext` | Composite: HasConfig + HasDatabase + HasEventBus |

**Registries:**

- `ExtensionRegistry` - Dynamic discovery via `inventory` crate
- `TypedExtensionRegistry` - Type-safe registry using `TypeId`
- `ExtensionBuilder` - Type-safe composition with dependency validation

**Discovery:**

```rust
use systemprompt_extension::*;

struct MyExtension;
impl Extension for MyExtension { ... }
register_extension!(MyExtension);

// At runtime
let registry = ExtensionRegistry::discover();
```

---

### Unified Path Constants

Path constants are centralized in `shared/models/src/paths/constants.rs` with re-exports in `infra/cloud/src/constants.rs` for backward compatibility.

**Constant modules:**

| Module | Purpose |
|--------|---------|
| `dir_names` | Directory names (`.systemprompt`, `profiles`, `docker`, `storage`) |
| `file_names` | File names (`profile.yaml`, `secrets.json`, `credentials.json`, etc.) |
| `cloud_container` | Container paths for Docker deployments (`/app`, `/app/bin`, etc.) |
| `storage` | Storage subdirectory structure |
| `build` | Build-related paths |

**Usage:**

```rust
use systemprompt_models::paths::constants::{dir_names, file_names};
use systemprompt_cloud::constants::storage;

let systemprompt_dir = root.join(dir_names::SYSTEMPROMPT);
let images_path = storage_root.join(storage::IMAGES);
```

**Storage structure:** `storage/files/{images,audio,video,documents,uploads}/`. Extensions declare paths via `required_storage_paths()`.

**Key rules:**
- Core owns top-level structure; extensions own subdirectories
- `profile.yaml` `paths.storage` points to root storage directory
- Always use `storage::*` constants, never hardcode paths

---

### Core Defaults Directory

The `defaults/` directory contains fallback templates, assets, and web content that extensions can override.

**Structure:** `defaults/{templates,assets,web}/`

**Access:** `AppPaths::get()?.system().default_templates()` (etc.)

**Priority:** Extension templates (500) override core defaults (1000). Path is derived from `{system_root}/core/defaults`.

---

### File Upload System

Handles file attachments in A2A messages via `FileUploadService` in `domain/files/`.

**Persistence Modes:**

| Mode | Path Pattern | Use Case |
|------|--------------|----------|
| `ContextScoped` | `contexts/{context_id}/{category}/{filename}` | Chat attachments (default) |
| `UserLibrary` | `users/{user_id}/{category}/{filename}` | Permanent files |
| `Disabled` | N/A | Skip persistence |

**Flow:** Upload → Validate (MIME/size) → Generate UUID → Write disk → SHA256 → DB metadata → Return URL

---

### AI Configuration Hierarchy

The AI generation system uses a strict 3-level configuration hierarchy:

```
Global (ai.yaml) → Agent (agents.yaml) → Tool (runtime override)
```

**Priority**: Tool > Agent > Global (highest to lowest)

| Level | Source | Scope |
|-------|--------|-------|
| Global | `ai.yaml` | All requests (default_provider, default_model, default_max_output_tokens) |
| Agent | `agents.yaml` metadata | All requests from this agent (provider, model, max_output_tokens) |
| Tool | `tool_model_overrides` | Specific tool executions |

**Canonical Execution Struct:**

All providers use the same `GenerationParams` struct:

```rust
pub struct GenerationParams<'a> {
    pub messages: &'a [AiMessage],
    pub model: &'a str,
    pub max_output_tokens: u32,
    pub sampling: Option<&'a SamplingParams>,
}
```

**Key Files:**

| File | Purpose |
|------|---------|
| `shared/models/src/services/ai.rs` | AiConfig (global defaults) |
| `shared/models/src/services/agent_config.rs` | AgentMetadataConfig (agent overrides) |
| `domain/ai/src/services/providers/provider_trait.rs` | GenerationParams (canonical struct) |
| `domain/agent/src/services/a2a_server/processing/ai_executor.rs` | resolve_provider_config() |

For complete documentation including YAML examples and data flow diagrams, see **[generation.md](generation.md)**.

---

### Multimodal AI Integration

The system supports sending images, audio, and video to AI providers (currently Gemini).

**Supported Media Types:**

| Category | MIME Types | Max Size |
|----------|------------|----------|
| Images | image/jpeg, image/png, image/gif, image/webp | 20MB |
| Audio | audio/wav, audio/mp3, audio/mpeg, audio/aiff, audio/aac, audio/ogg, audio/flac | 25MB |
| Video | video/mp4, video/mpeg, video/mov, video/avi, video/x-flv, video/mpg, video/webm, video/wmv, video/3gpp | 2GB |
| Text | text/plain, text/markdown, text/csv, text/html, text/xml, application/json, application/xml | N/A |

**Note:** Text files are base64-decoded and included as text content with filename metadata. Unsupported file types log a warning and are not sent to the AI.

**Content Flow:**

```
Message with file parts → ConversationService.extract_message_content()
                                ↓
                          Create AiContentPart::Image or AiContentPart::Audio
                                ↓
                          AiMessage { content, parts: Vec<AiContentPart> }
                                ↓
                          Gemini converter → GeminiPart::InlineData
                                ↓
                          Gemini API receives multimodal content
```

**Key Files:**

| File | Purpose |
|------|---------|
| `shared/models/src/ai/media_types.rs` | Supported MIME types and helper functions |
| `shared/models/src/ai/request.rs` | `AiContentPart` enum (Text, Image, Audio, Video) |
| `domain/agent/src/services/a2a_server/processing/conversation_service.rs` | Extract file parts from messages |
| `domain/ai/src/services/providers/gemini/converters.rs` | Convert to `GeminiPart::InlineData` |

**Usage Pattern:**

```rust
let (text, parts) = ConversationService::extract_message_content(&message);
let ai_message = AiMessage {
    role: MessageRole::User,
    content: text,
    parts,  // Vec<AiContentPart> - includes images/audio
};
```

---

### Product Binary Pattern

Products must own the final binary to include extension jobs via `inventory` static initialization.

**Why:** The `inventory` crate's `submit_job!()` registers jobs in static collectors, but statics are only linked if the crate is in the binary. Core's CLI doesn't link extensions.

**Product structure:**

| File | Purpose |
|------|---------|
| `src/lib.rs` | Facade re-exporting core + extensions |
| `src/main.rs` | Binary: `use my_product as _; systemprompt_cli::run().await` |
| `Cargo.toml` | `[[bin]]` target + all dependencies |

**Migration weights:** Core modules: 1-10, User extensions: 100+. Ensures core tables exist before extension tables.

---

### Schema Registration System

Each crate owns its database schemas via the Extension trait. Schemas are embedded at compile time using `include_str!()` within each crate's `src/extension.rs`.

**Extension-based schema registration:**

```rust
// crates/domain/users/src/extension.rs
use systemprompt_extension::prelude::*;

#[derive(Default)]
pub struct UsersExtension;

impl Extension for UsersExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "users",
            name: "Users",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn migration_weight(&self) -> u32 {
        10  // Lower weights run first
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline("users", include_str!("../schema/users.sql"))
                .with_required_columns(vec!["id".into(), "email".into()]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]  // No dependencies for base module
    }
}

register_extension!(UsersExtension);
```

**Migration weights (execution order):**

| Weight | Crate | Purpose |
|--------|-------|---------|
| 1 | database | PostgreSQL functions |
| 5 | logging | Log and analytics tables |
| 10 | users | Base user identity |
| 15 | analytics, files | Depend on users |
| 20 | ai, mcp, oauth | Depend on users |
| 25 | agent | Depends on users, oauth, mcp |
| 30 | content | Depends on users, analytics |
| 35 | scheduler | Depends on users |

**Schema discovery:**

```rust
use systemprompt_loader::ModuleLoader;

// Discover all extensions and their schemas
let extensions = ModuleLoader::discover_extensions();
let schemas = ModuleLoader::collect_extension_schemas();
```

**Benefits:**
- Compile-time SQL validation (missing file = compile error)
- Works when published to crates.io (schemas within crate boundary)
- Automatic discovery via `inventory` crate
- Dependency ordering via `migration_weight()`

**Adding schemas to a crate:**
1. Create SQL files in `{crate}/schema/`
2. Create `src/extension.rs` implementing `Extension` trait
3. Add `pub mod extension;` to `lib.rs`
4. Use `register_extension!()` macro for automatic discovery

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

### Execution Tracking System

The system uses multiple database tables to track different types of executions. This is separate from general logging (`tracing::*` → `logs` table).

**Tracking Tables:**

| Table | Purpose | Logged By |
|-------|---------|-----------|
| `mcp_tool_executions` | MCP tool call tracking | MCP Server only |
| `ai_requests` | AI provider request/response | AI Service |
| `ai_request_tool_calls` | Tool calls within AI requests | AI Service |
| `logs` | General application logs | `tracing::*` via DatabaseLayer |

**Request Flow and Logging Points:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. DIRECT MCP CALL (CLI or API)                                             │
│                                                                             │
│    CLI/API Request                                                          │
│         │                                                                   │
│         ▼                                                                   │
│    MCP Client (core)  ─────────────────►  MCP Server (extension)            │
│    [NO logging here]                      [LOGS to mcp_tool_executions]     │
│                                           - input, output, structured_content│
│                                           - timing, status, ai_tool_call_id │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ 2. AGENT TOOL USE (A2A message processing)                                  │
│                                                                             │
│    User Message                                                             │
│         │                                                                   │
│         ▼                                                                   │
│    AI Service ──────► [LOGS to ai_requests]                                 │
│    (planning)         - provider, model, messages, response                 │
│         │                                                                   │
│         ▼                                                                   │
│    Tool Executor ───► [LOGS to ai_request_tool_calls]                       │
│         │             - tool_name, arguments (linked to ai_request)         │
│         │                                                                   │
│         ▼                                                                   │
│    MCP Client ─────────────────────────►  MCP Server                        │
│    [NO logging]                           [LOGS to mcp_tool_executions]     │
│                                           - full structured_content         │
│         │                                                                   │
│         ▼                                                                   │
│    AI Service ──────► [LOGS to ai_requests]                                 │
│    (synthesis)        - final response generation                           │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ 3. AI REQUEST (direct LLM call without tools)                               │
│                                                                             │
│    Service Request                                                          │
│         │                                                                   │
│         ▼                                                                   │
│    AI Service ──────► [LOGS to ai_requests]                                 │
│                       - provider, model, input/output tokens                │
│                       - messages, response, timing                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Files:**

| Component | Location | Responsibility |
|-----------|----------|----------------|
| MCP Server logging | `extensions/mcp/*/src/server/mod.rs` | Logs all tool executions to `mcp_tool_executions` |
| MCP Client | `domain/mcp/src/services/client/mod.rs` | Calls MCP servers (no logging - server handles it) |
| AI request logging | `domain/ai/src/services/core/request_storage/` | Logs AI requests to `ai_requests` |
| Tool call logging | `domain/ai/src/repository/ai_requests/` | Logs tool calls to `ai_request_tool_calls` |
| General tracing | `infra/logging/src/` | Routes `tracing::*` to `logs` table |

**Linking Executions:**

Executions are linked via shared identifiers in `RequestContext`:

| Field | Purpose | Tables Using It |
|-------|---------|-----------------|
| `trace_id` | Distributed tracing across services | All tables |
| `context_id` | Conversation/session grouping | All tables |
| `task_id` | A2A task grouping | `mcp_tool_executions`, `ai_requests` |
| `ai_tool_call_id` | Links AI tool call to MCP execution | `ai_request_tool_calls` ↔ `mcp_tool_executions` |
| `mcp_execution_id` | Unique MCP execution identifier | `mcp_tool_executions`, `task_artifacts` |

**Important:** MCP tool execution logging happens **only in the MCP server** (extension layer), not in the MCP client (core). This ensures a single source of truth with complete data including `structured_content`.

---

### CLI Session Management

All CLI commands requiring cloud authentication use the `CloudContext` system for consistent credential and session handling.

**Architecture:**

```
~/.systemprompt/
  credentials.json     <- Cloud credentials (JWT token)
  tenants.json         <- Synced tenant data
  session.json         <- CLI session (context_id, session_id)
```

**Session Flow:**

```
CLI Command starts
    │
    ▼
CloudContext::new_authenticated()
    │ ← Load credentials.json (required)
    │ ← Load session.json (optional)
    ▼
get_or_create_request_context("agent-name")
    │ ← If session exists: reuse context_id/session_id
    │ ← If no session: call fetch_or_create_context() via API
    │ ← Save session.json
    ▼
Execute command with RequestContext
```

**Key Components:**

| Component | Location | Purpose |
|-----------|----------|---------|
| `CloudCredentials` | `infra/cloud/src/credentials.rs` | JWT token persistence (0o600 perms) |
| `CliSession` | `infra/cloud/src/cli_session.rs` | Session persistence (context_id, session_id) |
| `CloudContext` | `infra/cloud/src/context.rs` | Bundles credentials + session + API client |
| `CloudPath` | `infra/cloud/src/paths/cloud.rs` | Path resolution for cloud files |

**CLI Command Pattern:**

```rust
pub async fn execute(args: Args, config: &CliConfig) -> Result<CommandResult<Output>> {
    let mut cloud_ctx = CloudContext::new_authenticated()
        .context("Cloud authentication required. Run 'systemprompt cloud login'")?;

    let request_context = cloud_ctx
        .get_or_create_request_context("my-cli-command")
        .await
        .context("Failed to create request context")?;

    // Use request_context.auth_token() for API calls
    // Use request_context.context_id() for tracking
}
```

**Rules:**

| Rule | Description |
|------|-------------|
| Cloud auth required | All MCP and agent CLI commands require cloud login |
| Session reuse | Sessions are reused across CLI invocations |
| Context via API | Context IDs are created/fetched via Systemprompt API |
| Local persistence | Session data stored locally in JSON (not cloud) |
| Token fallback removed | No `--token` flag override - use cloud login |

---

### CLI Bootstrap System

The CLI uses a type-safe bootstrap system with compile-time dependency enforcement.

**Components:**

| Component | Location | Purpose |
|-----------|----------|---------|
| `CommandRequirements` | `entry/cli/src/requirements.rs` | Specifies what initialization a command needs |
| `HasRequirements` trait | Same file | Trait for commands to declare requirements |
| `BootstrapSequence<S>` | `shared/models/src/bootstrap/mod.rs` | Type-state pattern for safe initialization order |
| `bootstrap` module | `entry/cli/src/bootstrap.rs` | Bootstrap helper functions |

**Command Requirements:**

Commands declare their initialization needs via `HasRequirements`:

```rust
impl HasRequirements for Commands {
    fn requirements(&self) -> CommandRequirements {
        match self {
            Self::Cloud(cmd) => cmd.requirements(),
            Self::Setup(_) | Self::Session(_) => CommandRequirements::NONE,
            Self::Build(_) | Self::Extensions(_) => CommandRequirements::PROFILE_ONLY,
            Self::System(_) => CommandRequirements::PROFILE_AND_SECRETS,
            _ => CommandRequirements::FULL,
        }
    }
}
```

**Requirement presets:**

| Preset | Profile | Secrets | Paths |
|--------|---------|---------|-------|
| `NONE` | No | No | No |
| `PROFILE_ONLY` | Yes | No | No |
| `PROFILE_AND_SECRETS` | Yes | Yes | No |
| `FULL` | Yes | Yes | Yes |

**Type-safe bootstrap sequence:**

The `BootstrapSequence<S>` uses the type-state pattern to enforce initialization order at compile time:

```rust
BootstrapSequence::new()
    .with_profile(&path)?      // Returns BootstrapSequence<ProfileInitialized>
    .with_secrets()?           // Returns BootstrapSequence<SecretsInitialized>
    .with_paths()?             // Returns BootstrapSequence<PathsInitialized>
    .complete()                // Returns BootstrapComplete
```

Attempting to call `.with_secrets()` before `.with_profile()` is a compile-time error.

---

### Project Discovery

The `DiscoveredProject` and `UnifiedContext` types provide unified project root discovery and path resolution.

**Components:**

| Type | Location | Purpose |
|------|----------|---------|
| `DiscoveredProject` | `infra/cloud/src/paths/discovery.rs` | Discovers project root by walking up for `.systemprompt` |
| `UnifiedContext` | `infra/cloud/src/paths/context.rs` | Combines project discovery with cloud path resolution |

**Usage:**

```rust
use systemprompt_cloud::{DiscoveredProject, UnifiedContext};

// Simple discovery
if let Some(project) = DiscoveredProject::discover() {
    let creds = project.credentials_path();
    let session = project.session_path();
}

// Unified context with profile paths
let ctx = UnifiedContext::discover()
    .with_profile_paths(&profile_dir, creds_path, tenants_path);

let credentials = ctx.credentials_path();
let session = ctx.session_path();
```

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
| `shared/provider-contracts` | `systemprompt-provider-contracts` | Provider trait contracts (LlmProvider, ToolProvider, Job, etc.) |
| `shared/traits` | `systemprompt-traits` | Infrastructure traits, re-exports provider-contracts |
| `shared/template-provider` | `systemprompt-template-provider` | Template loading, re-exports provider-contracts |
| `shared/models` | `systemprompt-models` | Data models, config types |
| `shared/identifiers` | `systemprompt-identifiers` | Typed IDs |
| `shared/client` | `systemprompt-client` | HTTP client |
| `shared/extension` | `systemprompt-extension` | Extension framework |

### Infrastructure Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `infra/database` | `systemprompt-database` | SQLx abstraction |
| `infra/events` | `systemprompt-events` | Event bus, SSE |
| `infra/security` | `systemprompt-security` | JWT, auth utils |
| `infra/config` | `systemprompt-config` | Config loading |
| `infra/logging` | `systemprompt-logging` | Tracing setup |
| `infra/loader` | `systemprompt-loader` | File loading, module discovery |
| `infra/cloud` | `systemprompt-cloud` | Cloud API, tenants |

### Domain Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `domain/users` | `systemprompt-users` | User management |
| `domain/oauth` | `systemprompt-oauth` | OAuth2/OIDC |
| `domain/files` | `systemprompt-files` | File storage |
| `domain/analytics` | `systemprompt-analytics` | Metrics |
| `domain/content` | `systemprompt-content` | Content management |
| `domain/ai` | `systemprompt-ai` | LLM integration |
| `domain/mcp` | `systemprompt-mcp` | MCP protocol |
| `domain/agent` | `systemprompt-agent` | A2A protocol |
| `domain/templates` | `systemprompt-templates` | Template registry and rendering |

### Application Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `app/scheduler` | `systemprompt-scheduler` | Job scheduling |
| `app/generator` | `systemprompt-generator` | Static site gen |
| `app/sync` | `systemprompt-sync` | Sync services |
| `app/runtime` | `systemprompt-runtime` | AppContext, lifecycle |

### Entry Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `entry/cli` | `systemprompt-cli` | Command-line interface |
| `entry/api` | `systemprompt-api` | HTTP server |

### Facade Layer

| Crate | Package Name | Purpose |
|-------|--------------|---------|
| `systemprompt/` (root) | `systemprompt` | Public API for crates.io |

