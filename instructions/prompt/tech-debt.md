# Tech Debt & Code Quality Audit

> Comprehensive audit of a crate for Rust coding standards, anti-patterns, tech debt, and architectural consistency. This codebase is being prepared for crates.io publication.

---

## Input

- **Crate Path:** `{crate_path}` (e.g., `crates/domain/agent`)
- **Standards:** `instructions/prompt/rust.md`
- **Architecture:** `instructions/information/architecture.md`
- **Boundaries:** `instructions/information/boundaries.md`
- **Linting:** `instructions/prompt/lint.md`

---

## Audit Process

### Phase 1: Discovery

1. Read `{crate_path}/Cargo.toml` to determine:
   - Crate name and layer (shared/infra/domain/app/entry)
   - Dependencies - check for forbidden cross-layer imports
   - Feature flags and optional dependencies

2. List all `.rs` files in `{crate_path}/src/`

3. Read `instructions/information/architecture.md` to understand layer rules

---

### Phase 2: Architectural Compliance

#### 2.1 Layer Verification

Determine the crate's layer and verify compliance:

| Layer | Location | Allowed Dependencies |
|-------|----------|---------------------|
| Shared | `crates/shared/*` | Other shared crates only, NO I/O, NO SQL |
| Infra | `crates/infra/*` | Shared only |
| Domain | `crates/domain/*` | Shared + Infra only, NO cross-domain |
| App | `crates/app/*` | Shared + Infra + Domain |
| Entry | `crates/entry/*` | All layers |

**Check Cargo.toml for violations:**

```
# Forbidden patterns by layer:
Shared: Any sqlx, tokio runtime, file I/O crates
Infra: systemprompt-{domain_crate} imports
Domain: systemprompt-{other_domain} imports (except via traits)
```

#### 2.2 Required Structure (Domain Layer Only)

Domain crates MUST have:

```
{crate}/
  schema/           # SQL schema files
  src/
    lib.rs          # Public API
    error.rs        # Domain-specific errors
    models/         # Or re-export from shared
    repository/     # Data access layer
      mod.rs
      {entity}_repository.rs
    services/       # Business logic
      mod.rs
      {entity}_service.rs
```

**Flag if missing:** repository/, services/, error.rs

#### 2.3 Dependency Direction

Scan imports for violations:

```rust
// FORBIDDEN - Upward dependencies
use systemprompt_api::*;      // Infra importing Entry
use systemprompt_runtime::*;  // Domain importing App
use systemprompt_agent::*;    // Infra importing Domain

// FORBIDDEN - Cross-domain (unless via trait)
use systemprompt_users::*;    // In systemprompt-oauth
use systemprompt_oauth::*;    // In systemprompt-users
```

---

### Phase 3: Rust Standards Compliance

#### 3.1 Zero-Tolerance Violations

Scan ALL `.rs` files for these FORBIDDEN constructs:

| Pattern | Search Regex | Resolution |
|---------|--------------|------------|
| Inline comments | `^\s*//[^!/]` | DELETE - code documents itself |
| Doc comments | `^\s*///` or `^\s*//!` | DELETE - no rustdoc |
| TODO/FIXME/HACK | `TODO\|FIXME\|HACK\|XXX` | Fix or remove |
| `unsafe` blocks | `unsafe\s*\{` | Remove - forbidden |
| `unwrap()` | `\.unwrap\(\)` | Use `?` or `expect()` with message |
| `panic!()` | `panic!\(` | Return `Result` |
| `todo!()` | `todo!\(` | Implement |
| `unimplemented!()` | `unimplemented!\(` | Implement |
| `#[cfg(test)]` | `#\[cfg\(test\)\]` | Move to `crates/tests/` |
| `println!` in lib | `println!\(` | Use `tracing::` |
| `eprintln!` | `eprintln!\(` | Use `tracing::error!` |
| `dbg!()` | `dbg!\(` | Remove debug macro |

#### 3.2 Typed Identifiers

All ID fields MUST use typed wrappers from `systemprompt_identifiers`:

```rust
// WRONG - raw String IDs
pub id: String
pub user_id: String
pub task_id: String

// RIGHT - typed identifiers
pub id: TaskId
pub user_id: UserId
pub context_id: ContextId
```

**Available types:** `SessionId`, `UserId`, `AgentId`, `TaskId`, `ContextId`, `TraceId`, `ClientId`, `AgentName`, `AiToolCallId`, `McpExecutionId`, `SkillId`, `SourceId`, `CategoryId`

**Scan for:** `pub.*_id:\s*String` or `id:\s*String`

#### 3.3 SQLX Macro Enforcement

All SQL queries MUST use compile-time verified macros:

| Allowed | Forbidden |
|---------|-----------|
| `sqlx::query!()` | `sqlx::query()` |
| `sqlx::query_as!()` | `sqlx::query_as()` |
| `sqlx::query_scalar!()` | `sqlx::query_scalar()` |

**Scan for:** `sqlx::query\(` or `sqlx::query_as\(` (without `!`)

#### 3.4 Repository Pattern

Services MUST NOT execute SQL directly:

```rust
// WRONG - SQL in service
impl UserService {
    async fn get_user(&self) {
        sqlx::query_as!(...).fetch_one(&self.pool).await
    }
}

// RIGHT - service calls repository
impl UserService {
    async fn get_user(&self) -> Result<User> {
        self.user_repository.find_by_id(id).await
    }
}
```

**Scan for:** `sqlx::` in `*_service.rs` files

#### 3.5 Error Handling Anti-Patterns

##### Silent Error Swallowing

| Pattern | Search | Resolution |
|---------|--------|------------|
| `.ok()` on Result | `\.ok\(\)` | Propagate with `?` or log first |
| `let _ = result` | `let _ =` | Handle error explicitly |
| `Err(_) =>` | `Err\(_\)\s*=>` | Handle specific variants |
| `unwrap_or_default()` | `unwrap_or_default\(\)` | Fail explicitly |
| Error log then Ok | `error!.*\n.*Ok\(` | Propagate error after logging |

**Acceptable `.ok()` only in:**
1. Cleanup during error path already returning Err
2. Transaction rollback with logging

##### Missing Error Context

```rust
// WRONG
.map_err(|e| e)?

// RIGHT
.map_err(|e| ServiceError::Database(e))?
.context("Failed to fetch user")?
```

#### 3.6 DateTime Violations

| Violation | Resolution |
|-----------|------------|
| `NaiveDateTime` | Use `DateTime<Utc>` |
| `TIMESTAMP` in SQL | Use `TIMESTAMPTZ` |
| String formatting for DB | Use native types |

#### 3.7 Builder Pattern Requirements

Types with 3+ fields OR mixed required/optional fields MUST use builder pattern:

```rust
// WRONG - optional fields in constructor
pub fn new(a: T, b: Option<U>, c: Option<V>) -> Self

// RIGHT - builder pattern
pub fn builder(a: T) -> Builder
impl Builder {
    pub fn with_b(self, b: U) -> Self
    pub fn build(self) -> Target
}
```

**Scan for:** Functions with 3+ `Option<T>` parameters

#### 3.8 Logging Violations

| Violation | Resolution |
|-----------|------------|
| `LogService::new()` | Use `req_ctx.span().enter()` |
| `LogService::system()` | Use `SystemSpan::new("name").enter()` |
| `logger.info().await.ok()` | Use `tracing::info!()` |
| Orphan `tracing::*` without span | Enter span first |
| Format strings over fields | Use structured fields |

```rust
// WRONG
tracing::info!("Created user {}", user.id);

// RIGHT
tracing::info!(user_id = %user.id, "Created user");
```

---

### Phase 4: Code Quality Metrics

#### 4.1 Size Limits

| Metric | Limit | Action |
|--------|-------|--------|
| File length | 300 lines | Split into modules |
| Function length | 75 lines | Extract helpers |
| Parameters | 5 | Use config struct or builder |
| Cognitive complexity | 15 | Refactor logic |

#### 4.2 Naming Conventions

| Prefix | Expected Return |
|--------|-----------------|
| `get_` | `Result<T>` - fails if missing |
| `find_` | `Result<Option<T>>` - may not exist |
| `list_` | `Result<Vec<T>>` |
| `create_` | `Result<T>` or `Result<Id>` |
| `update_` | `Result<T>` or `Result<()>` |
| `delete_` | `Result<()>` |
| `is_`/`has_` | `bool` |

**Flag:** `get_` returning `Option<T>` or `find_` returning `T` (not Option)

#### 4.3 Idiomatic Patterns

**Anti-patterns to flag:**

| Anti-Pattern | Idiomatic |
|--------------|-----------|
| `if let Some(x) = opt { x } else { default }` | `opt.unwrap_or(default)` |
| `match opt { Some(x) => Some(f(x)), None => None }` | `opt.map(f)` |
| `if condition { Some(x) } else { None }` | `condition.then(\|\| x)` |
| Manual loop building Vec | Iterator `.collect()` |
| Nested `if let`/`match` | Combine with `and_then`, `map` |

---

### Phase 5: Tech Debt Indicators

#### 5.1 Dead Code

```bash
# Search for:
- Unused imports (cargo check --message-format=json)
- Unused functions (#[allow(dead_code)])
- Commented-out code
- Empty impl blocks
- Unreachable match arms
```

#### 5.2 Hardcoded Values

| Pattern | Resolution |
|---------|------------|
| Magic numbers | Define constants |
| Hardcoded strings | Use constants or config |
| Inline paths | Use `constants::` module |
| Fallback defaults | Fail explicitly or use config |

```rust
// WRONG
let timeout = 30;
let path = "/var/www/storage";

// RIGHT
const DEFAULT_TIMEOUT_SECS: u64 = 30;
use constants::storage::STORAGE_ROOT;
```

#### 5.3 Config Anti-Patterns

```rust
// WRONG - env var fallback
let path = std::env::var("PATH").unwrap_or_default();

// WRONG - silent fallback
let path = config.path.clone().unwrap_or_else(|| "/default".into());

// RIGHT - profile-derived, fail if missing
let path = &config.path;
```

#### 5.4 Overly Complex Code

Flag these for refactoring:

- Functions with >3 levels of nesting
- Match expressions with >7 arms
- Chain of >5 `and_then`/`map` calls
- Trait bounds spanning >2 lines
- Type aliases hiding complex generics without justification

---

### Phase 6: Verification Commands

Run these commands and record results:

```bash
# Clippy - must pass with zero warnings
cargo clippy -p {crate_name} -- -D warnings

# Formatting
cargo fmt -p {crate_name} -- --check

# Check for forbidden patterns
rg '\.unwrap\(\)' {crate_path}/src --type rust
rg '\.ok\(\)' {crate_path}/src --type rust -g '!*test*'
rg 'let _ =' {crate_path}/src --type rust
rg 'TODO|FIXME|HACK' {crate_path}/src --type rust
rg '^\s*//[^!/]' {crate_path}/src --type rust
rg 'panic!\(' {crate_path}/src --type rust
rg 'unwrap_or_default\(\)' {crate_path}/src --type rust

# Check for raw String IDs
rg 'pub.*_id:\s*String' {crate_path}/src --type rust
rg 'id:\s*String' {crate_path}/src --type rust

# Check for non-macro SQL
rg 'sqlx::query\(' {crate_path}/src --type rust
rg 'sqlx::query_as\(' {crate_path}/src --type rust
```

---

## Output Format

Generate `{crate_path}/status.md` with:

```markdown
# {crate_name} Tech Debt Audit

**Layer:** {Shared | Infrastructure | Domain | Application | Entry}
**Audited:** {YYYY-MM-DD}
**Verdict:** {CLEAN | NEEDS_WORK | CRITICAL}

---

## Summary

| Category | Status | Issues |
|----------|--------|--------|
| Architecture | ✅/❌ | {count} |
| Rust Standards | ✅/❌ | {count} |
| Code Quality | ✅/❌ | {count} |
| Tech Debt | ✅/❌ | {count} |

**Total Issues:** {N}

---

## Critical Violations

{Violations that MUST be fixed before crates.io publication}

| File:Line | Violation | Category | Severity |
|-----------|-----------|----------|----------|
| `src/foo.rs:42` | `unwrap()` usage | Rust Standards | Critical |

---

## Warnings

{Issues that should be addressed but don't block publication}

| File:Line | Issue | Category |
|-----------|-------|----------|
| `src/bar.rs:15` | Function exceeds 75 lines | Code Quality |

---

## Tech Debt Items

{Areas identified for future improvement}

| Location | Description | Priority |
|----------|-------------|----------|
| `services/` | Missing builder pattern for XConfig | Medium |

---

## Commands Executed

\`\`\`
cargo clippy -p {crate_name} -- -D warnings  # {PASS/FAIL}
cargo fmt -p {crate_name} -- --check          # {PASS/FAIL}
\`\`\`

---

## Required Actions

### Before crates.io Publication

1. {Critical fix 1}
2. {Critical fix 2}

### Recommended Improvements

1. {Non-blocking improvement}

---

## Verdict Criteria

**CLEAN**: Zero critical violations, ready for crates.io
**NEEDS_WORK**: Minor issues, can publish with warnings
**CRITICAL**: Blocking issues, must resolve before publication
```

---

## Checklist Summary

Before marking CLEAN, verify ALL items pass:

### Zero-Tolerance (Publication Blockers)

- [ ] Zero inline comments (`//`) except rare `//!` module docs
- [ ] Zero doc comments (`///`)
- [ ] Zero `unwrap()` calls
- [ ] Zero `panic!()`, `todo!()`, `unimplemented!()`
- [ ] Zero `unsafe` blocks
- [ ] Zero raw String IDs (all use typed identifiers)
- [ ] Zero non-macro SQLX calls (`query` without `!`)
- [ ] Zero SQL in service files (repository pattern enforced)
- [ ] Zero forbidden dependencies for layer
- [ ] Zero `#[cfg(test)]` modules (tests in separate crate)
- [ ] Zero `println!`/`eprintln!`/`dbg!` in library code
- [ ] Zero TODO/FIXME/HACK comments
- [ ] Clippy passes with `-D warnings`
- [ ] Formatting passes `cargo fmt --check`

### Code Quality (Should Fix)

- [ ] All files under 300 lines
- [ ] All functions under 75 lines
- [ ] All functions have ≤5 parameters
- [ ] No silent error swallowing (`.ok()` without context)
- [ ] No `unwrap_or_default()` usage
- [ ] No hardcoded fallback values
- [ ] No direct `env::var()` access

### Best Practices (Recommended)

- [ ] Builder pattern for complex types (3+ fields)
- [ ] Correct naming conventions (`get_` vs `find_` vs `list_`)
- [ ] Structured logging with `tracing::` and proper spans
- [ ] Idiomatic combinators over imperative control flow
- [ ] Domain-specific error types with `thiserror`
- [ ] Proper error context propagation
