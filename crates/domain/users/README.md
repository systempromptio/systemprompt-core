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

# systemprompt-users

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-users.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-users.svg">
    <img alt="systemprompt-users terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-users.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-users.svg?style=flat-square)](https://crates.io/crates/systemprompt-users)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-users?style=flat-square)](https://docs.rs/systemprompt-users)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

User management for systemprompt.io AI governance infrastructure. 6-tier RBAC, sessions, IP bans, and role-scoped access control for the MCP governance pipeline. Provides user CRUD, session management, bulk operations, and anonymous user lifecycle management.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [Compliance](https://systemprompt.io/features/compliance)

This crate provides user management functionality including:

- User CRUD operations with typed identifiers
- Session management (list, end, cleanup, existence checks)
- Role-based access control with policy-aware promotion/demotion
- API key issuance, hashing, and verification
- Device certificate enrollment and rotation
- IP banning with expiration and metadata tracking
- Anonymous user lifecycle management and scheduled cleanup
- Bulk operations and aggregate statistics

## Usage

```toml
[dependencies]
systemprompt-users = "0.11.0"
```

```rust
use systemprompt_database::DbPool;
use systemprompt_users::{UserService, UserRole, UserStatus};

let user_service = UserService::new(&db_pool)?;

let user = user_service.find_by_email("user@example.com").await?;

let admins = user_service.find_by_role(UserRole::Admin).await?;

let stats = user_service.get_stats().await?;
```

## Directory Structure

```
src/
├── lib.rs                              # Crate docs, public exports
├── error.rs                            # UserError enum, Result / UserResult aliases
├── extension.rs                        # UsersExtension (schema + job registration)
├── models/
│   └── mod.rs                          # User, UserSession, UserActivity, UserStats,
│                                       # UserApiKey, UserDeviceCert, NewApiKey, UserExport
├── repository/
│   ├── mod.rs                          # UserRepository facade, MAX_PAGE_SIZE constant
│   ├── api_key.rs                      # API key persistence and lookup
│   ├── device_cert.rs                  # Device certificate persistence
│   ├── banned_ip/
│   │   ├── mod.rs                      # BannedIpRepository
│   │   ├── types.rs                    # BannedIp, BanDuration, BanIpParams,
│   │   │                               # BanIpWithMetadataParams
│   │   ├── queries.rs                  # ban_ip, unban_ip, is_banned, get_ban,
│   │   │                               # cleanup_expired
│   │   └── listing.rs                  # list_active_bans, list_bans_by_source,
│   │                                   # count_active_bans
│   └── user/
│       ├── mod.rs                      # Module exports
│       ├── find.rs                     # find_by_id, find_by_email, find_by_name,
│       │                               # find_by_role
│       ├── list.rs                     # list, search, count, bulk operations
│       ├── stats.rs                    # count_by_status, count_by_role, get_stats
│       ├── operations.rs               # create, update_*, delete, cleanup_old_anonymous
│       ├── merge.rs                    # merge_users, MergeResult
│       └── session.rs                  # list_sessions, end_session, end_all_sessions,
│                                       # session_exists
├── services/
│   ├── mod.rs                          # Service exports
│   ├── admin_service.rs                # UserAdminService, PromoteResult, DemoteResult
│   ├── api_key_service.rs              # ApiKeyService, IssueApiKeyParams,
│   │                                   # API_KEY_PREFIX
│   ├── device_cert_service.rs          # DeviceCertService, EnrollDeviceCertServiceParams
│   ├── user_provider.rs                # UserProviderImpl wrapper for trait-based access
│   └── user/
│       ├── mod.rs                      # UserService — primary service
│       └── provider.rs                 # UserProvider / RoleProvider impls
└── jobs/
    ├── mod.rs                          # Job exports
    └── cleanup_anonymous_users.rs      # CleanupAnonymousUsersJob (retention window)
```

## Public Exports

### Models

- `User` — Core user entity with id, name, email, roles, status
- `UserSession` — Session with timestamps and device info
- `UserActivity` — User activity summary (last active, counts)
- `UserWithSessions` — User with active session count
- `UserStats` — Aggregate statistics (totals, breakdowns)
- `UserCountBreakdown` — Counts by status and role
- `UserApiKey` — Stored API key record
- `NewApiKey` — Plaintext key returned at issuance
- `UserDeviceCert` — Stored device certificate record
- `UserExport` — Export-friendly user representation

### Enums

- `UserStatus` — Active, Suspended, Deleted (re-exported from `systemprompt-models`)
- `UserRole` — Admin, User, Anonymous (re-exported from `systemprompt-models`)

### Services

- `UserService` — Primary service implementing `UserProvider` and `RoleProvider`
- `UserAdminService` — Admin operations (promote, demote)
- `ApiKeyService` — Issue, hash, and verify API keys
- `DeviceCertService` — Enroll and rotate device certificates
- `UserProviderImpl` — Wrapper for trait-based dependency injection

### Repositories

- `UserRepository` — User database operations
- `BannedIpRepository` — IP ban management

### Types

- `UpdateUserParams` — Multi-field user update struct
- `MergeResult` — Result of merging two users
- `IssueApiKeyParams` — Parameters for `ApiKeyService::issue`
- `EnrollDeviceCertServiceParams` — Parameters for `DeviceCertService::enroll`
- `CreateApiKeyParams` — Repository-level API key creation parameters
- `EnrollDeviceCertParams` — Repository-level device cert parameters
- `BanDuration` — Hours, Days, or Permanent
- `BanIpParams` — Basic ban parameters
- `BanIpWithMetadataParams` — Ban with offense tracking
- `BannedIp` — Active ban record
- `PromoteResult` / `DemoteResult` — Outcomes of admin role transitions
- `API_KEY_PREFIX` — Canonical user-facing key prefix

### Extension

- `UsersExtension` — Schema and job registration entry point

### Traits (re-exported)

- `UserProvider` — User lookup and creation
- `RoleProvider` — Role management

### Error Handling

- `UserError` — Domain-specific errors (`NotFound`, `EmailAlreadyExists`, …)
- `Result<T>` / `UserResult<T>` — Aliases for `std::result::Result<T, UserError>`

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | `DbPool` for database access |
| `systemprompt-extension` | `Extension` trait for schema/job registration |
| `systemprompt-traits` | `UserProvider`, `RoleProvider`, `Job` traits |
| `systemprompt-identifiers` | `UserId`, `SessionId` typed identifiers (sqlx feature) |
| `systemprompt-models` | `UserRole`, `UserStatus` enums |
| `systemprompt-provider-contracts` | Job registration macro |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-users)** · **[docs.rs](https://docs.rs/systemprompt-users)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
