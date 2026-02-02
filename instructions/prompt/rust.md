# systemprompt.io Rust Standards

**systemprompt.io is a world-class Rust programming brand.** Every Rust file must be instantly recognizable as idiomatic Rust as Steve Klabnik would write it.

Run `cargo clippy --workspace -- -D warnings` and `cargo fmt --all` after changes.

---

## 1. Idiomatic Rust

Prefer iterator chains, combinators, and pattern matching over imperative control flow.

```rust
let name = request.name.as_deref().map(str::trim);
let value = opt.unwrap_or_else(|| compute_default());
let result = input.ok_or_else(|| Error::Missing)?;

let valid_items: Vec<_> = items
    .iter()
    .filter(|item| item.is_active())
    .map(|item| item.to_dto())
    .collect();
```

| Anti-Pattern | Idiomatic |
|--------------|-----------|
| `if let Some(x) = opt { x } else { default }` | `opt.unwrap_or(default)` |
| `match opt { Some(x) => Some(f(x)), None => None }` | `opt.map(f)` |
| `if condition { Some(x) } else { None }` | `condition.then(\|\| x)` |
| Nested `if let` / `match` | Combine with `and_then`, `map`, `ok_or` |
| Manual loops building `Vec` | Iterator chains with `collect()` |

---

## 2. Limits

| Metric | Limit |
|--------|-------|
| Source file length | 300 lines |
| Cognitive complexity | 15 |
| Function length | 75 lines |
| Parameters | 5 |

---

## 3. Forbidden Constructs

| Construct | Resolution |
|-----------|------------|
| `unsafe` | Remove - forbidden in this codebase |
| `unwrap()` | Use `?`, `ok_or_else()`, or `expect()` with message |
| `unwrap_or_default()` | Fail explicitly - never use fuzzy defaults |
| `panic!()` / `todo!()` / `unimplemented!()` | Return `Result` or implement |
| Inline comments (`//`) | Delete - code documents itself through naming |
| Doc comments (`///`, `//!`) | Delete - no rustdoc (rare `//!` module docs excepted) |
| TODO/FIXME/HACK comments | Fix immediately or don't write |
| Tests in source files (`#[cfg(test)]`) | Move to `crates/tests/` |
| Raw `env::var()` | Use `Config::init()` / `AppContext` |
| Magic numbers/strings | Use constants or enums |
| Commented-out code | Delete - git has history |

---

## 4. Mandatory Patterns

### Typed Identifiers

All identifier fields use wrappers from `systemprompt_identifiers`:

```rust
use systemprompt_identifiers::{TaskId, UserId};
pub struct Task { pub id: TaskId, pub user_id: UserId }
```

Available: `SessionId`, `UserId`, `AgentId`, `TaskId`, `ContextId`, `TraceId`, `ClientId`, `AgentName`, `AiToolCallId`, `McpExecutionId`, `SkillId`, `SourceId`, `CategoryId`, `ArtifactId`.

### Logging

All logging via `tracing`. No `println!` in library code.

**Request-scoped (handlers, services):**
```rust
let _guard = req_ctx.span().enter();
tracing::info!(user_id = %user.id, "Created user");
```

**System/background (schedulers, startup):**
```rust
let _guard = SystemSpan::new("scheduler").enter();
tracing::info!("Running cleanup job");
```

**Adding context mid-request:**
```rust
let span = req_ctx.span();
span.record_task_id(&task_id);
let _guard = span.enter();
```

Use structured fields: `tracing::info!(user_id = %id, "msg")` not `tracing::info!("msg {}", id)`.

### Repository Pattern

Services NEVER execute queries directly. All SQL in repositories using SQLX macros:

```rust
pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT id, email, name FROM users WHERE email = $1", email)
        .fetch_optional(&**self.pool)
        .await
}
```

| Allowed | Forbidden |
|---------|-----------|
| `sqlx::query!()` | `sqlx::query()` |
| `sqlx::query_as!()` | `sqlx::query_as()` |
| `sqlx::query_scalar!()` | `sqlx::query_scalar()` |

The `!` suffix enables compile-time verification.

### Repository Constructors

**Reference Pattern (repositories):**
```rust
impl UserRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self { pool: db.pool_arc()? })
    }
}
```

**Owned Pattern (services/composites):**
```rust
impl TaskService {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}
```

| Pattern | Parameter Name |
|---------|---------------|
| Reference | `db: &DbPool` |
| Owned | `db_pool: DbPool` |

### Error Handling

Use domain-specific errors with `thiserror`. `anyhow` only at application boundaries:

```rust
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("User not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
}
```

Log errors once at handling boundary, not at every propagation point.

### DateTime

| Layer | Type |
|-------|------|
| Rust | `DateTime<Utc>` |
| PostgreSQL | `TIMESTAMPTZ` |

Never use `NaiveDateTime` or `TIMESTAMP`.

### Builder Pattern

**Required** for types with 3+ fields OR mixed required/optional fields.

```rust
impl AiRequest {
    pub fn builder(messages: Vec<AiMessage>, provider: &str, model: &str,
                   max_tokens: u32, ctx: RequestContext) -> AiRequestBuilder {
        AiRequestBuilder::new(messages, provider, model, max_tokens, ctx)
    }
}

let request = AiRequest::builder(messages, "gemini", "gemini-2.5-flash", 8192, ctx)
    .with_sampling(params)
    .with_tools(tools)
    .build();
```

| Rule | Description |
|------|-------------|
| Required fields in `new()` | All non-optional fields as constructor parameters |
| Optional fields via `with_*()` | Each optional field gets a builder method |
| `build()` consumes builder | Returns final struct |
| No `Default` for complex types | Explicit construction prevents invalid states |

---

## 5. Naming

### Functions

| Prefix | Returns |
|--------|---------|
| `get_` | `Result<T>` - fails if missing |
| `find_` | `Result<Option<T>>` - may not exist |
| `list_` | `Result<Vec<T>>` |
| `create_` | `Result<T>` or `Result<Id>` |
| `update_` | `Result<T>` or `Result<()>` |
| `delete_` | `Result<()>` |
| `is_` / `has_` | `bool` |

### Variables

| Type | Name |
|------|------|
| Database pool | `db_pool` |
| Repository | `{noun}_repository` |
| Service | `{noun}_service` |

### Abbreviations

Allowed: `id`, `uuid`, `url`, `jwt`, `mcp`, `a2a`, `api`, `http`, `json`, `sql`, `ctx`, `req`, `res`, `msg`, `err`, `cfg`

---

## 6. Silent Error Anti-Patterns

These patterns silently swallow errors, making debugging impossible:

| Pattern | Resolution |
|---------|------------|
| `.ok()` on Result | Use `?` or `map_err()` to propagate with context |
| `let _ = result` | Handle error explicitly or use `?` |
| `match { Err(_) => default }` | Propagate error or log with `tracing::error!` |
| `filter_map(\|e\| e.ok())` | Log failures before filtering |
| Error log then `Ok()` | Propagate the error after logging |

**Acceptable `.ok()` usage:**

1. **Cleanup in error paths** - when already returning an error:
```rust
if let Err(e) = operation().await {
    cleanup().await.ok();
    return Err(e);
}
```

2. **Parse with logged warning:**
```rust
serde_json::from_str(s).map_err(|e| {
    tracing::warn!(error = %e, "Parse failed");
    e
}).ok()
```

**Detection commands:**
```bash
rg '\.ok\(\)' --type rust -g '!*test*'
rg 'let _ =' --type rust -g '!*test*'
rg 'unwrap_or_default\(\)' --type rust -g '!*test*'
```

---

## 7. Multi-Process Broadcasting

Events from agent/worker processes must go through HTTP webhook to API process:

```
Agent Process → HTTP POST /webhook → API Process → CONTEXT_BROADCASTER → SSE clients
```

Use `BroadcastClient` trait:
- `create_webhook_broadcaster(token)` - for agent services
- `create_local_broadcaster()` - for API routes (same process)
