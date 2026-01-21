# Module Boundary Guidelines

This document defines architectural boundaries and acceptable patterns for cross-module dependencies in systemprompt-core.

**See also**: [architecture.md](./architecture.md) for crate taxonomy (shared/infra/domain/app/entry layers).

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

**Key files:**
- `crates/domain/agent/src/services/agent_orchestration/process.rs`
- `crates/domain/mcp/src/services/process/spawner.rs`
- `crates/shared/models/src/secrets.rs`

### 6. Module System

Modules are defined in Rust code at `crates/infra/loader/src/modules/`. Each module uses `include_str!()` to embed SQL schemas at compile time.

**Modules vs Extensions:**

| Aspect | Modules | Extensions |
|--------|---------|------------|
| Discovery | `modules::all()` in loader | `inventory` crate + `register_extension!()` |
| Schema embedding | `include_str!()` in module definition | `SchemaSource::Inline` in `impl Extension` |
| Location | `crates/infra/loader/src/modules/` | User project or domain crate |
| Purpose | Core domain schemas (users, oauth, etc.) | User customization, plugins |

### 7. Extension Linkage via Product Binary

Extensions register jobs, schemas, and routes via `inventory` macros. These are static initializers that only execute if the crate is linked into the final binary.

**Key rule:** Core's CLI binary does NOT link extension crates. Products must own the binary.

```rust
use my_product as _;  // Forces linkage

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    systemprompt_cli::run().await
}
```

---

## Layer Dependencies

### Allowed Dependency Direction

```
Entry (api, cli) → App (runtime, scheduler, sync, generator)
                 ↓
           Domain (agent, ai, mcp, oauth, users, files, content, analytics, templates)
                 ↓
           Infra (database, events, security, config, logging, loader, cloud)
                 ↓
           Shared (models, traits, identifiers, extension, provider-contracts, client, template-provider)
```

### Forbidden Dependencies

| Layer | Cannot Depend On |
|-------|------------------|
| Shared | Any systemprompt crate (except within shared/) |
| Infra | domain/, app/, entry/ |
| Domain | Other domain crates, app/, entry/ |
| App | entry/ |

### Acceptable Cross-Domain Dependencies

Domain crates using another domain's public API is acceptable when:
- Dependency is downward (orchestration layer using lower-level service)
- Uses public repository/service API, not internal types
- No circular dependencies exist

**Example:** `agent → mcp` is correct (agent orchestrates MCP tools)

---

## Current Architecture Boundaries

### Agent Module (`crates/domain/agent/`)

**Depends on:**
- `systemprompt-oauth` - Authentication
- `systemprompt-users` - User lookup
- `systemprompt-logging` - Logging
- `systemprompt-database` - Database pool
- `systemprompt-mcp` - Tool orchestration (legitimate downward dependency)

**Key services:**
- `services/a2a_server/` - A2A protocol server
- `services/agent_orchestration/` - Agent process management
- `repository/` - Task, context, message persistence

### API Module (`crates/entry/api/`)

**Depends on:** All domain and app crates (entry layer wires everything)

**Key components:**
- `src/routes/` - HTTP route handlers
- `src/services/` - API-specific services
- Uses `AppContext` for dependency access

### Scheduler Module (`crates/app/scheduler/`)

**Depends on:** Domain crates via `Job` trait abstractions

**Pattern:** Jobs are registered via `inventory` crate. Scheduler discovers and executes them without importing domain-specific logic directly.

---

## Design Patterns

### Config Validation

Unified startup validation via `DomainConfig` trait and `StartupValidator`:

```rust
pub trait DomainConfig: Send + Sync {
    fn domain_id(&self) -> &'static str;
    fn load(&mut self, config: &Config) -> Result<(), DomainConfigError>;
    fn validate(&self) -> Result<ValidationReport, DomainConfigError>;
}
```

**Rules:**
- Each domain owns its semantic validation logic
- `StartupValidator` in `app/runtime` orchestrates all validators
- Extensions register via `ConfigExtensionTyped` trait
- Core never references specific extensions

### Error Handling

Each domain defines its own error types. Use `thiserror` for derivation. Convert at boundaries using `From` implementations.

### Service Instantiation

Services receive dependencies through constructors, not global state:

```rust
// GOOD: Explicit dependencies
pub fn new(db: DbPool, config: &AiConfig) -> Self

// BAD: Service locator
pub fn new(app_context: &AppContext) -> Self  // Hides true dependencies
```

---

## Layer Violation Prevention

### Infra → Domain (MUST prevent)

Infrastructure crates (`database`, `logging`, `events`) must NEVER import domain crates:
- No `systemprompt-agent` in infra
- No `systemprompt-ai` in infra
- No `systemprompt-mcp` in infra

### Domain → Domain (Allowed if downward)

- `agent` → `mcp` ✓ (orchestration uses tool service)
- `ai` → `mcp` ✓ (AI uses MCP for tool execution)
- `agent` → `ai` - Only if agent orchestrates AI (check actual usage)

### Validation Commands

```bash
# Check for forbidden dependencies
grep "systemprompt-agent" crates/infra/*/Cargo.toml  # Should be empty
grep "systemprompt-ai" crates/infra/*/Cargo.toml     # Should be empty

# Verify domain isolation
grep "systemprompt-" crates/domain/*/Cargo.toml | grep -v "systemprompt-models\|systemprompt-traits\|systemprompt-identifiers\|systemprompt-database\|systemprompt-events"
```

---

## Summary

### Core Rules

1. **Downward dependencies only** - Higher layers depend on lower layers
2. **No cross-domain imports** except via public API for orchestration
3. **Config profiles required** - No env var fallbacks
4. **Explicit subprocess propagation** - Pass secrets directly
5. **Single implementations = concrete types** - Traits only for polymorphism

### Acceptable Patterns

- Cross-module repository usage (correct pattern for data access)
- Domain module depending on lower domain (agent → mcp)
- Entry layer importing all layers (wiring point)
- Using `AppContext` in entry layer routes

### Patterns to Avoid

- Infra importing domain
- Service locator via `AppContext` in domain logic
- Silent config fallbacks
- Global singletons for testable services
