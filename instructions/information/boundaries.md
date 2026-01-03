# Module Boundary Guidelines

This document defines architectural boundaries and acceptable patterns for cross-module dependencies in systemprompt-core.

**See also**: [architecture.md](./architecture.md) for crate taxonomy (shared/infra/domain/app/entry layers) and migration plan.

---

## Guiding Principles

### 1. Repositories Are Public API

Using a repository from another module is the **correct pattern** for cross-module data access:
- Repositories are intentionally exposed via `pub mod repository`
- This is idiomatic Rust - no need for extra abstraction layers
- Dependencies are clear: caller depends on callee's repository

### 2. Downward Dependencies Are Fine

Dependencies are acceptable when:
- They flow downward (higher-level → lower-level)
- There are no circular dependencies
- The boundary is clear (using public API)

Example: `agent → mcp` is correct because agent orchestrates MCP tools.

### 3. Avoid Over-Abstraction

Do NOT add traits just for the sake of abstraction:
- If only one implementation exists, use the concrete type
- Traits add complexity without benefit for single implementations
- This is not Java - avoid dependency injection patterns

### 4. Config Profiles Are Mandatory

All code must use config profiles - no environment variable fallbacks:
- `Config::from_profile()` is the only way to build configuration
- Missing paths cause **startup errors**, not runtime fallbacks
- Each domain validates its config via `DomainConfig` trait
- Extensions validate via `ConfigExtensionTyped` trait
- Validation is **always blocking** - no `--force` bypass

**Anti-patterns to avoid**:
```rust
// BAD: Direct env var access
let path = std::env::var("SYSTEMPROMPT_WEB_PATH").unwrap_or_default();

// BAD: Silent fallback
let path = config.web_path.clone().unwrap_or_else(|| "/default".into());

// GOOD: Use profile-derived config
let path = &config.web_path;  // Required field, validated at startup
```

### 5. Subprocess Config/Secrets Propagation

When spawning subprocesses (agents, MCP servers), config and secrets MUST be passed explicitly:

**Required env vars for ALL subprocesses:**
- `SYSTEMPROMPT_PROFILE` - Path to profile.yaml
- `JWT_SECRET` - JWT signing secret (passed directly, no file discovery)
- `DATABASE_URL` - Database connection string

**Rules:**
- Parent MUST pass secrets explicitly - no fuzzy profile discovery in subprocesses
- Subprocesses MUST prioritize `JWT_SECRET` env var over file loading
- All processes in the system MUST use identical JWT secrets for token validation
- Never use `if let Ok(...)` patterns for secrets - fail loudly if missing

**Anti-patterns to avoid**:
```rust
// BAD: Optional profile passing
if let Ok(profile_path) = ProfileBootstrap::get_path() {
    child_command.env("SYSTEMPROMPT_PROFILE", profile_path);
}

// BAD: Not passing JWT_SECRET
command.envs(std::env::vars())  // Inherits vars but JWT_SECRET may not exist

// GOOD: Explicit, required passing
let profile_path = ProfileBootstrap::get_path()?;  // Fail if missing
let jwt_secret = SecretsBootstrap::jwt_secret()?;  // Fail if missing
command
    .env("SYSTEMPROMPT_PROFILE", profile_path)
    .env("JWT_SECRET", jwt_secret);
```

**Key files:**
- `crates/domain/agent/src/services/agent_orchestration/process.rs`
- `crates/domain/mcp/src/services/process/spawner.rs`
- `crates/shared/models/src/secrets.rs`

### 6. Module System

Modules are defined in Rust code at `crates/infra/loader/src/modules/`. Each module uses `include_str!()` to embed SQL schemas at compile time.

**Module pattern:**
```rust
// crates/infra/loader/src/modules/users.rs
pub fn define() -> Module {
    Module {
        name: "users".into(),
        schemas: Some(vec![
            ModuleSchema {
                table: "users".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/users.sql").into()
                ),
                required_columns: vec!["id".into()],
            },
        ]),
        // ...
    }
}
```

**Modules vs Extensions:**

| Aspect | Modules | Extensions |
|--------|---------|------------|
| Discovery | `modules::all()` in loader | `inventory` crate + `register_extension!()` |
| Schema embedding | `include_str!()` in module definition | `SchemaSource::Inline` in `impl Extension` |
| Location | `crates/infra/loader/src/modules/` | User project or domain crate |
| Purpose | Core domain schemas (users, oauth, etc.) | User customization, plugins |

**Both use the same `SchemaSource` enum:**
```rust
pub enum SchemaSource {
    Inline(String),    // Embedded SQL
    File(PathBuf),     // Path to SQL file (dev only)
}
```

**Adding a new module:**
1. Create SQL files in `domain/{name}/schema/`
2. Create `modules/{name}.rs` with `pub fn define() -> Module`
3. Add `mod {name};` and call in `modules/mod.rs`

**Adding a new extension (user project):**
```rust
use systemprompt_extension::*;

struct MyExtension;

impl Extension for MyExtension {
    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::inline(
            "my_table",
            include_str!("../schema/my_table.sql"),
        )]
    }
}

register_extension!(MyExtension);
```

### 7. Extension Linkage via Product Binary

Extensions register jobs, schemas, and routes via `inventory` macros (`submit_job!`, `register_extension!`). These registrations are static initializers that only execute if the crate is linked into the final binary.

**Key rule:** Core's CLI binary does NOT link extension crates. Products must own the binary.

**Correct pattern:**

```rust
use my_product as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    systemprompt_cli::run().await
}
```

**Why `use ... as _;`:**

The underscore import forces the crate to be linked without bringing names into scope. This triggers:
1. Static initializers in the crate
2. `inventory` collection of all registered items
3. Job/extension discovery at runtime

**Anti-patterns:**

| Anti-pattern | Why it fails |
|--------------|--------------|
| Using core's binary directly | Extension jobs not discovered - core binary doesn't link extensions |
| Not importing the facade | Extensions not linked, `inventory` never collects their registrations |

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    systemprompt_cli::run().await
}
```

**Verification:**

```bash
./target/debug/systemprompt services scheduler list
```

---

## 1. Dependency Direction

### 1.1 Agent Module Dependencies (RESOLVED)

**Current state**: Agent depends on:
- systemprompt-core-oauth (authentication)
- systemprompt-core-users (user lookup)
- systemprompt-core-logging (logging)
- systemprompt-core-database (database pool)
- systemprompt-core-mcp (tool orchestration) ✓ LEGITIMATE
- systemprompt-core-system (core types)

**Removed** (were unused):
- systemprompt-core-ai

**Why MCP is acceptable**: Agent orchestrates AI agents that use MCP tools. This is a downward dependency with clear boundaries.

---

### 1.2 API Routes Import Repositories Directly

**Locations**:
- `crates/modules/api/src/api/routes/stream/contexts.rs:8` - imports `ContextRepository`
- `crates/modules/api/src/api/routes/stream/a2a.rs` - imports agent repositories
- `crates/modules/api/src/api/routes/stream/agui.rs` - imports agent services

**Why problematic**: Routes should depend on service abstractions, not repository implementations. This bypasses the service layer and couples HTTP handlers to data access patterns.

---

### 1.3 API Proxy Imports Concrete Registries

**Location**: `crates/modules/api/src/services/proxy/auth.rs:5-8`

```rust
use systemprompt_core_agent::services::registry::AgentRegistry;
use systemprompt_core_mcp::McpServerRegistry;
use systemprompt_core_oauth::services::AuthService;
```

**Why problematic**: Proxy authorization should depend on registry traits, not concrete implementations. Testing and swapping implementations becomes difficult.

---

### 1.4 TUI Depends on Business Logic Modules

**Location**: `crates/modules/tui/Cargo.toml:23-31`

TUI imports from 7 modules:
- systemprompt-core-system
- systemprompt-core-agent
- systemprompt-core-mcp
- systemprompt-core-logging
- systemprompt-core-database
- systemprompt-core-users
- systemprompt-core-oauth

**Why problematic**: A terminal UI should use API clients to communicate with the backend, not import business logic modules directly. This couples the UI to internal implementations.

---

### 1.5 Scheduler Imports Domain Modules

**Location**: `crates/modules/scheduler/Cargo.toml:34-41`

```toml
systemprompt-core-ai = { path = "../ai" }
systemprompt-core-content = { path = "../content" }
systemprompt-core-agent = { path = "../agent" }
systemprompt-core-users = { path = "../users" }
systemprompt-core-files = { path = "../files" }
```

**Why problematic**: The scheduler should receive job definitions through abstractions (traits), not import domain modules directly. This makes the scheduler a coupling point for all domain logic.

---

## 2. Mixed Concerns and Unclear Responsibilities

### 2.1 Core Module is a Dumping Ground

**Location**: `crates/modules/core/src/lib.rs`

The core module contains unrelated concerns:
- Authentication (`src/auth/`)
- Authorization (`src/security/`)
- Analytics (`src/repository/analytics/`, `src/services/analytics/`)
- JWT/Security (`src/services/security/`)
- Token extraction (`src/services/extraction/`)
- Configuration (`src/services/config/`)
- Infrastructure (`src/services/infrastructure/`)
- Validation (`src/services/validation/`)
- Health checks (`src/services/health/`)
- Sync/deployment (`src/services/sync/`)
- Broadcasting (`src/services/broadcasters/`)
- Middleware (`src/middleware/`)
- Repository layer (`src/repository/`)

**Why problematic**: "Core" becomes a catch-all for anything cross-cutting. Changes to analytics affect security; changes to broadcasting affect validation. The module has many reasons to change.

---

### 2.2 Core Re-exports 100+ Items

**Location**: `crates/modules/core/src/lib.rs:12-45`

```rust
pub use systemprompt_models::{
    ApiError, ApiResponse, AuthError, AuthenticatedUser, BaseRole, CollectionResponse, ColumnInfo,
    Config, DatabaseInfo, DiscoveryResponse, Link, ModelConfig, OAuthClientConfig,
    OAuthServerConfig, PaginationInfo, QueryResult, RequestContext, ResponseLinks, ResponseMeta,
    SingleResponse, TableInfo, TaskMessage, TaskRecord, BEARER_PREFIX,
};
```

**Why problematic**: Creates confusion about ownership. Other modules import from core to get model types, creating a false dependency on core when they only need models.

---

### 2.3 Scheduler Does Business Logic

**Location**: `crates/modules/scheduler/src/services/`

The scheduler contains:
- AI evaluation (`jobs/evaluate_conversations.rs`)
- Content ingestion (`jobs/content_ingestion.rs`)
- File ingestion (`jobs/file_ingestion.rs`)
- Static site generation (`static_content/` - markdown, templates, sitemap, prerendering)
- Publishing workflows (`jobs/publish_content.rs`)

**Why problematic**: Scheduler should only schedule and run jobs. The job implementations (content processing, AI evaluation) should live in their respective domain modules.

---

### 2.4 Blog Module Contains Analytics

**Location**: `crates/modules/blog/src/repository/`

Blog includes:
- Content CRUD
- Link analytics (`link_analytics_repository.rs`)
- Search functionality (`search_repository.rs`)

**Why problematic**: Link analytics is a separate concern from content management. Should be its own module.

---

### 2.5 Agent Module Contains MCP Orchestration (ACCEPTABLE)

**Location**: `crates/modules/agent/src/services/external_integrations/mcp/`

Agent contains MCP-specific:
- Tool loading
- Server discovery
- MCP orchestration logic

**Why this is acceptable**: Agent is an orchestration layer that coordinates AI agents with MCP tools. Using MCP's public repository and service APIs is the correct pattern. No trait abstraction needed - this would be over-engineering.

---

## 3. Missing Abstractions

### 3.1 No Base Repository Trait

**Locations** (32 repository implementations):
- `crates/modules/agent/src/repository/context/mod.rs`
- `crates/modules/core/src/repository/analytics/session.rs`
- `crates/modules/oauth/src/repository/client_repository/mod.rs`
- `crates/modules/users/src/repository/mod.rs`
- `crates/modules/blog/src/repository/content.rs`
- `crates/modules/mcp/src/repository/tool_usage.rs`
- (26 more...)

All follow identical patterns: `DbPool` -> `PgPool` -> `sqlx::query!` -> `Result<T, RepositoryError>`

**Why problematic**: Each module reimplements the same patterns. No code reuse, inconsistent error handling, duplicate boilerplate.

---

### 3.2 Services Are Concrete, Not Trait-Based

**Locations**:
- `crates/modules/ai/src/services/core/ai_service/service.rs` - `AiService` is a struct
- `crates/modules/oauth/src/services/mod.rs` - `AuthService` is a struct
- `crates/modules/agent/src/services/registry/mod.rs` - `AgentRegistry` is a struct

**Why problematic**: Without traits, modules must depend on concrete implementations. Cannot swap implementations for testing or different environments.

---

### 3.3 Tool Executor Pattern (CLARIFIED)

**Previous guidance** suggested creating a `ToolExecutor` trait. This is **not recommended**.

**Current guidance**:
- If agent needs AI functionality, import from AI module directly
- If there's only one implementation, use the concrete type
- Traits are for polymorphism, not for the sake of abstraction

**Note**: The AI dependency was removed from agent as it was unused. If AI integration is needed in the future, direct import is acceptable.

---

## 4. Duplicate Implementations

### 4.1 Multiple Service Duplications

**Locations**:
- 3 implementations of MCP-related services across modules
- 3 implementations of agent-related services
- 2 implementations of AI service instantiation

**Example**: `crates/modules/scheduler/src/services/jobs/evaluate_conversations.rs:34`
```rust
let ai_service = AiService::new(&app_context, &services_config.ai)?;
```

**Why problematic**: Each module independently instantiates shared services rather than receiving them through dependency injection.

---

### 4.2 Config Validation (RESOLVED)

**Solution**: Unified startup validation via `DomainConfig` trait and `StartupValidator`.

**Pattern**:
```rust
// shared/traits/src/domain_config.rs
pub trait DomainConfig: Send + Sync {
    fn domain_id(&self) -> &'static str;
    fn load(&mut self, config: &Config) -> Result<(), DomainConfigError>;
    fn validate(&self) -> Result<ValidationReport, DomainConfigError>;
}

// Each domain implements DomainConfig
impl DomainConfig for WebConfigValidator { ... }
impl DomainConfig for ContentConfigValidator { ... }
impl DomainConfig for McpConfigValidator { ... }
```

**Key rules**:
- Each domain owns its semantic validation logic
- `StartupValidator` orchestrates all validators at startup
- Extensions register via `ConfigExtensionTyped` trait
- Core never references specific extensions

---

### 4.3 Error Types Duplicated

**Locations**: Each module defines custom error types with similar patterns.

**Why problematic**: No unified error hierarchy. Errors converted at module boundaries instead of being composed.

---

## 5. Global State Issues

### 5.1 Broadcaster Singletons

**Location**: `crates/modules/core/src/lib.rs:30-34`

```rust
pub use services::broadcasters::{
    A2A_BROADCASTER, AGUI_BROADCASTER, CONTEXT_BROADCASTER, ...
};
```

**Why problematic**: Global singletons make testing difficult. Cannot inject mock broadcasters. Hidden dependencies throughout codebase.

---

### 5.2 AppContext Passed Everywhere

**Location**: 50+ files pass `AppContext` as parameter

**Example files**:
- `crates/modules/api/src/services/server/lifecycle/agents.rs`
- `crates/modules/tui/src/services/cloud_api.rs`
- `crates/modules/scheduler/src/services/jobs/evaluate_conversations.rs`

**Why problematic**: `AppContext` becomes a service locator anti-pattern. Hides true dependencies. Makes refactoring difficult as everything depends on it.

---

## 6. Boundary Violations in Request Handling

### 6.1 Routes Construct Domain Objects

**Location**: `crates/modules/api/src/api/routes/stream/contexts.rs:35-45`

```rust
let snapshot_data: Vec<ContextSummary> = contexts_with_stats
    .iter()
    .map(|c| ContextSummary {
        context_id: c.context_id.clone(),
        name: c.name.clone(),
        // ...
    })
    .collect();
```

**Why problematic**: API routes know internal structure of domain objects. Should use service methods or mappers.

---

### 6.2 Cross-Module Type Dependencies

**Locations**:
- `crates/modules/core/src/lib.rs:28` - Re-exports `Database, DbPool` from database module
- `crates/modules/ai/src/models/providers/openai.rs:9` - Re-exports `ModelConfig` from core
- `crates/modules/agent/src/services/shared/auth.rs:3` - Re-exports `JwtClaims` from oauth

**Why problematic**: Creates transitive dependencies. Modules appear independent but are coupled through re-exports.

---

## 7. Files with Most Boundary Violations

| File | Cross-Module Imports | Issue |
|------|---------------------|-------|
| `api/src/services/server/lifecycle/agents.rs` | 7 | Orchestration point, high coupling |
| `tui/src/services/cloud_api.rs` | 6 | UI importing business logic |
| `api/src/services/server/lifecycle/scheduler.rs` | 6 | Direct scheduler access |
| `agent/src/services/external_integrations/mcp/orchestration/loader.rs` | 6 | Cross-domain coupling |
| `tui/src/services/agent_discovery.rs` | 5 | UI importing domain services |
| `scheduler/src/services/jobs/evaluate_conversations.rs` | 5 | Job doing business logic |

---

## 8. Circular Dependency Risks

### 8.1 Conceptual Circular Dependencies

While Cargo prevents compile-time circular crate dependencies, conceptual circles exist:

**Pattern**: A depends on B's concrete type, B conceptually needs A's functionality

**Current Examples**:
- `agent` imports `ai` for tool execution
- `ai` may need agent context for multi-turn conversations
- Solved by: `ai` implementing `ToolExecutor` trait that `agent` depends on

**Location**: `crates/modules/agent/src/services/a2a_server/processing/ai_executor.rs`

---

### 8.2 Re-export Circles

**Pattern**: Module A re-exports from B, Module B imports from A

**Location**: `crates/modules/core/src/lib.rs`

Core re-exports from `systemprompt_models`, and if models ever imported from core, this would create a conceptual circle.

**Why problematic**: Creates hidden coupling and makes it unclear which module owns which types.

---

### 8.3 Layer Violations That Could Become Circles

**Foundation → Domain** (MUST be prevented):
- `database` and `log` must never import `agent`, `ai`, `blog`, `mcp`

**Infrastructure → Integration** (MUST be prevented):
- `core`, `oauth`, `users` must never import `api`, `tui`, `scheduler`

**Domain Imports** (ACCEPTABLE if downward):
- `agent` → `mcp` ✓ (agent orchestrates MCP tools - downward dependency)
- `ai` → `mcp` ✓ (AI uses MCP for tool execution - downward dependency)

**Key rule**: Downward dependencies using public APIs (repositories, services) are acceptable. Only circular dependencies are violations.

---

## Summary by Severity

### Critical (Blocks Clean Architecture)
- Core module as dumping ground → **Planned**: decompose per [architecture.md](./architecture.md#core-module-decomposition)
- Foundation modules importing domain modules (circular risk)

### High (Significant Technical Debt)
- Scheduler containing business logic (jobs should be in domain modules)
- TUI depending on business logic modules (should use API client)
- Global broadcaster singletons (testing difficulty)

### Medium (Code Quality Issues)
- Error types duplicated
- Service instantiation duplicated

### Resolved / Acceptable
- Agent depending on MCP ✓ (downward dependency, uses public API)
- Agent depending on AI/Blog ✓ (removed - were unused)
- Cross-module repository usage ✓ (correct pattern for data access)
- Domain module dependencies ✓ (acceptable if downward, no cycles)
- Config validation ✓ (unified via `DomainConfig` trait and `StartupValidator`)
- Config profile enforcement ✓ (profiles required, no env var fallbacks)
