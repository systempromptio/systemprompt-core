<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-models

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-models — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-models.svg?style=flat-square)](https://crates.io/crates/systemprompt-models)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-models?style=flat-square)](https://docs.rs/systemprompt-models)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Foundation data models for systemprompt.io AI governance infrastructure. Shared DTOs, config, and domain types consumed by every layer of the MCP governance pipeline. Includes API models, authentication types, configuration, database models, and service-layer error handling.

**Layer**: Shared — foundational types/traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides common data models, error types, and repository patterns used throughout systemprompt.io. It includes API models, authentication types, configuration, database models, and service-layer error handling.

## Architecture

### `api` — API Response Models

Standard JSON API response structures.

### `auth` — Authentication Models

User identity, roles, and auth error types.

### `config` — Configuration Models

Config struct and provider integration.

### `errors` — Error Handling

Layered error types with automatic conversions:

```
RepositoryError → ServiceError → ApiError → HTTP Response
```

### `repository` — Repository Patterns

Service lifecycle trait, `WhereClause` query builder, and repository macros.

### `execution` — Execution Context

`RequestContext` and related types for request-scoped state propagation.

## Usage

```toml
[dependencies]
systemprompt-models = "0.2.1"
```

### `api` — API Response Models

```rust
use systemprompt_models::{ApiResponse, ApiError, ErrorCode};

let response = ApiResponse::success(data);
let error = ApiError::not_found("User not found");
```

### `auth` — Authentication Models

```rust
use systemprompt_models::{AuthenticatedUser, BaseRole, AuthError};

let user = AuthenticatedUser {
    id: "user-123".to_string(),
    username: "alice".to_string(),
    roles: vec![BaseRole::Admin],
    scopes: vec!["admin".to_string()],
};
```

### `config` — Configuration Models

```rust
use systemprompt_models::Config;
use systemprompt_traits::ConfigProvider;

let config = Config::from_env()?;
let db_url = config.database_url(); // ConfigProvider trait
```

### `errors` — Error Handling

**`RepositoryError`** — Database/repository layer errors:
```rust
use systemprompt_models::RepositoryError;

let err = RepositoryError::NotFound("user-123".to_string());
// Automatically converts to ApiError
let api_err: ApiError = err.into();
```

**`ServiceError`** — Business logic layer errors:
```rust
use systemprompt_models::ServiceError;

let err = ServiceError::Validation("Invalid email".to_string());
let api_err: ApiError = err.into(); // Converts to HTTP 400
```

### `repository` — Repository Patterns

**Service Lifecycle Trait:**
```rust
use systemprompt_models::ServiceLifecycle;

#[async_trait]
impl ServiceLifecycle for MyServiceRepository {
    async fn get_running_services(&self) -> Result<Vec<ServiceRecord>, RepositoryError> { ... }
    async fn mark_crashed(&self, service_id: &str) -> Result<(), RepositoryError> { ... }
    async fn update_status(&self, service_id: &str, status: &str) -> Result<(), RepositoryError> { ... }
}
```

**Query Builder:**
```rust
use systemprompt_models::WhereClause;

let (clause, params) = WhereClause::new()
    .eq("status", "active")
    .is_not_null("pid")
    .build();

let query = format!("SELECT * FROM services {}", clause);
```

**Repository Macros:**
```rust
use systemprompt_models::impl_repository_base;

impl_repository_base!(MyRepository, DbPool, db_pool);

// Expands to:
// impl Repository for MyRepository {
//     type Pool = DbPool;
//     type Error = RepositoryError;
//     fn pool(&self) -> &Self::Pool { &self.db_pool }
// }
```

### `execution` — Execution Context

```rust
use systemprompt_models::RequestContext;

let req_ctx = RequestContext {
    session_id: "session-123".into(),
    trace_id: "trace-456".into(),
    user_id: "user-789".into(),
    context_id: "ctx-000".into(),
    task_id: None,
    ai_tool_call_id: None,
    client_id: None,
    auth_token: None,
    user: None,
    start_time: std::time::Instant::now(),
    user_type: UserType::AdminUser,
};
```

### AgentConfig

```rust
use systemprompt_models::config::AgentConfig;

fn main() -> anyhow::Result<()> {
    let yaml = r#"
        id: developer_agent
        name: Developer
        description: Writes and reviews code
    "#;
    let agent: AgentConfig = serde_yaml::from_str(yaml)?;
    println!("loaded agent: {}", agent.id);
    Ok(())
}
```

## Error Handling Pattern

systemprompt.io uses a layered error handling approach:

### Layer 1: Repository (Database)

```rust
use systemprompt_traits::RepositoryError;

async fn get_user(&self, id: &str) -> Result<User, RepositoryError> {
    sqlx::query_as(...)
        .fetch_optional(self.pool().pool())
        .await?
        .ok_or_else(|| RepositoryError::NotFound(format!("User {}", id)))
}
```

### Layer 2: Service (Business Logic)

```rust
use systemprompt_models::ServiceError;

async fn create_user(&self, data: CreateUser) -> Result<User, ServiceError> {
    if data.email.is_empty() {
        return Err(ServiceError::Validation("Email required".into()));
    }

    self.repo.create_user(data)
        .await
        .map_err(|e| e.into()) // RepositoryError → ServiceError
}
```

### Layer 3: API (HTTP)

```rust
use systemprompt_models::ApiError;

async fn create_user_handler(
    State(service): State<UserService>,
    Json(data): Json<CreateUser>,
) -> Result<Json<User>, ApiError> {
    let user = service.create_user(data)
        .await
        .map_err(|e| e.into())?; // ServiceError → ApiError

    Ok(Json(user))
}
```

## Repository Pattern

All repositories should implement the `Repository` trait from `systemprompt-traits`:

```rust
use systemprompt_traits::{Repository, RepositoryError};
use systemprompt_database::DbPool;

pub struct UserRepository {
    db_pool: DbPool,
}

impl Repository for UserRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}

impl UserRepository {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        sqlx::query_as::<_, User>(GET_USER_QUERY)
            .bind(id)
            .fetch_optional(self.pool().pool())
            .await
            .map_err(|e| e.into())
    }
}

const GET_USER_QUERY: &str = "SELECT * FROM users WHERE id = ?";
```

## Query Helpers

### WhereClause Builder

```rust
use systemprompt_models::WhereClause;

let (clause, params) = WhereClause::new()
    .eq("status", "active")
    .is_not_null("deleted_at")
    .like("name", "%john%")
    .in_list("role", vec!["admin".into(), "user".into()])
    .build();

// clause = "WHERE status = ? AND deleted_at IS NOT NULL AND name LIKE ? AND role IN (?, ?)"
// params = vec!["active", "%john%", "admin", "user"]
```

### Repository Macros

```rust
// Base trait implementation
impl_repository_base!(UserRepository, DbPool, db_pool);

// Query execution
let users = repository_query!(
    self.pool(),
    "SELECT * FROM users WHERE status = ?",
    "active"
)?;

// Execute statement
repository_execute!(
    self.pool(),
    "UPDATE users SET status = ? WHERE id = ?",
    "inactive",
    user_id
)?;
```

## Module Models

```rust
use systemprompt_models::{Module, ModuleType, ServiceCategory};

let module = Module {
    id: "mod-123".to_string(),
    name: "my-module".to_string(),
    version: "1.0.0".to_string(),
    display_name: "My Module".to_string(),
    category: ServiceCategory::Core,
    module_type: ModuleType::Regular,
    enabled: true,
    config: HashMap::new(),
    ..Default::default()
};
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `web` | No | Axum `IntoResponse` implementations |

## Dependencies

- `serde` / `serde_json` — Serialization
- `anyhow` / `thiserror` — Error handling
- `chrono` / `uuid` — Common types
- `axum` — Request types (optional, with `web` feature)
- `async-trait` — Async traits
- `systemprompt-traits` — Core trait definitions
- `systemprompt-identifiers` — Typed identifiers
- `systemprompt-extension` — Extension framework
- `systemprompt-provider-contracts` — Provider trait definitions

## Best Practices

### 1. Use Shared Error Types

```rust
// Good
async fn my_repo_method(&self) -> Result<Data, RepositoryError> { ... }

// Avoid
async fn my_repo_method(&self) -> Result<Data, anyhow::Error> { ... }
```

### 2. Layer Your Errors

```rust
// Repository layer
Result<T, RepositoryError>

// Service layer
Result<T, ServiceError>

// API layer
Result<T, ApiError>
```

### 3. Use Query Builders

```rust
// Good
let (clause, params) = WhereClause::new().eq("status", status).build();

// Avoid — SQL injection risk
let clause = format!("WHERE status = '{}'", status);
```

### 4. Implement Repository Trait

```rust
// Good — consistent pattern
impl Repository for MyRepository { ... }

// Avoid — no trait, inconsistent
impl MyRepository {
    pub fn get_pool(&self) -> &DbPool { ... }
}
```

## Testing

Mock repositories using traits:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockUserRepository {
        users: Vec<User>,
    }

    impl Repository for MockUserRepository {
        type Pool = ();
        type Error = RepositoryError;
        fn pool(&self) -> &Self::Pool { &() }
    }

    #[tokio::test]
    async fn test_user_service() {
        let repo = MockUserRepository { users: vec![] };
        let service = UserService::new(repo);
        // Test service logic
    }
}
```

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-models)** · **[docs.rs](https://docs.rs/systemprompt-models)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
