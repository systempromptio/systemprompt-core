<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-users

Core user management module for systemprompt.io.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-users.svg)](https://crates.io/crates/systemprompt-users)
[![Documentation](https://docs.rs/systemprompt-users/badge.svg)](https://docs.rs/systemprompt-users)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the Domain layer in the systemprompt.io architecture.**

This crate provides user management functionality including:

- User CRUD operations with typed identifiers
- Session management (list, end, cleanup)
- Role-based access control (admin, user, anonymous)
- IP banning with expiration and metadata tracking
- Anonymous user lifecycle management
- Bulk operations for user administration

## Directory Structure

```
src/
├── lib.rs                              # Public exports
├── error.rs                            # UserError enum, Result type alias
├── models/
│   └── mod.rs                          # User, UserSession, UserActivity, UserStats, UserExport
├── repository/
│   ├── mod.rs                          # UserRepository struct, MAX_PAGE_SIZE constant
│   ├── banned_ip/
│   │   ├── mod.rs                      # BannedIpRepository struct
│   │   ├── types.rs                    # BannedIp, BanDuration, BanIpParams
│   │   ├── queries.rs                  # ban_ip, unban_ip, is_banned, get_ban, cleanup_expired
│   │   └── listing.rs                  # list_active_bans, list_bans_by_source, count_active_bans
│   └── user/
│       ├── mod.rs                      # Module exports
│       ├── find.rs                     # find_by_id, find_by_email, find_by_name, find_by_role
│       ├── list.rs                     # list, search, count, bulk operations
│       ├── stats.rs                    # count_by_status, count_by_role, get_stats
│       ├── operations.rs               # create, update_*, delete, cleanup_old_anonymous
│       ├── merge.rs                    # merge_users, MergeResult
│       └── session.rs                  # list_sessions, end_session, end_all_sessions
├── services/
│   ├── mod.rs                          # Service exports
│   ├── admin_service.rs                # UserAdminService, PromoteResult, DemoteResult
│   ├── user_provider.rs                # UserProviderImpl wrapper for trait-based access
│   └── user/
│       ├── mod.rs                      # UserService - primary service
│       └── provider.rs                 # UserProvider, RoleProvider trait implementations
└── jobs/
    ├── mod.rs                          # Job exports
    └── cleanup_anonymous_users.rs      # CleanupAnonymousUsersJob (30-day cleanup)
```

## Public Exports

### Models

- `User` - Core user entity with id, name, email, roles, status
- `UserSession` - Session with timestamps and device info
- `UserActivity` - User activity summary (last active, counts)
- `UserWithSessions` - User with active session count
- `UserStats` - Aggregate statistics (totals, breakdowns)
- `UserCountBreakdown` - Counts by status and role
- `UserExport` - Export-friendly user representation

### Enums

- `UserStatus` - Active, Suspended, Deleted (re-exported from systemprompt-models)
- `UserRole` - Admin, User, Anonymous (re-exported from systemprompt-models)

### Services

- `UserService` - Primary service implementing `UserProvider` and `RoleProvider`
- `UserAdminService` - Admin operations (promote, demote)
- `UserProviderImpl` - Wrapper for trait-based dependency injection

### Repositories

- `UserRepository` - User database operations
- `BannedIpRepository` - IP ban management

### Types

- `UpdateUserParams` - Multi-field update struct
- `MergeResult` - Result of merging two users
- `BanDuration` - Hours, Days, or Permanent
- `BanIpParams` - Basic ban parameters
- `BanIpWithMetadataParams` - Ban with offense tracking
- `BannedIp` - Active ban record

### Traits (re-exported)

- `UserProvider` - User lookup and creation
- `RoleProvider` - Role management

### Error Handling

- `UserError` - Domain-specific errors (NotFound, EmailAlreadyExists, etc.)
- `Result<T>` - Type alias for `std::result::Result<T, UserError>`

## Usage

```rust
use systemprompt_database::DbPool;
use systemprompt_users::{UserService, UserRole, UserStatus};

let user_service = UserService::new(&db_pool)?;

let user = user_service.find_by_email("user@example.com").await?;

let admins = user_service.find_by_role(UserRole::Admin).await?;

let stats = user_service.get_stats().await?;
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | DbPool for database access |
| `systemprompt-traits` | UserProvider, RoleProvider, Job traits |
| `systemprompt-identifiers` | UserId, SessionId typed identifiers |
| `systemprompt-models` | UserRole, UserStatus enums |
| `systemprompt-provider-contracts` | Job registration macro |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-users = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
