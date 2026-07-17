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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Your users, your database, your access rules. Role-based access control (`UserRole`: Admin, User, Anonymous), sessions, API keys, device certificates, federated identities, and IP bans, all held in your PostgreSQL rather than a third-party directory.

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
systemprompt-users = "0.21"
```

```rust
use systemprompt_database::DbPool;
use systemprompt_users::{UserService, UserRole, UserStatus};

let user_service = UserService::new(&db_pool)?;

let user = user_service.find_by_email("user@example.com").await?;

let admins = user_service.find_by_role(UserRole::Admin).await?;

let stats = user_service.get_stats().await?;
```

## Module Layout

| Module | Purpose |
|--------|---------|
| `models/` | `User`, `UserSession`, `UserActivity`, `UserStats`, `UserApiKey`, `UserDeviceCert`, and related records. |
| `repository/` | Compile-time-verified persistence: `user/` (find, list, stats, operations, merge, session), `api_key`, `device_cert`, `federated_identity`, and `banned_ip/`. |
| `services/` | `UserService` (primary), `UserAdminService`, `ApiKeyService`, and `DeviceCertService`. |
| `jobs/` | `CleanupAnonymousUsersJob` scheduled anonymous-user cleanup. |

Schema DDL lives in `schema/*.sql` (`users`, `user_sessions`, `user_api_keys`, `user_device_certs`, `federated_identities`, `banned_ips`, and the analytics views) with migrations in `schema/migrations/`.

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
