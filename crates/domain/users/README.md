# systemprompt-core-users

User identity and management module.

## Directory Structure

```
src/
├── lib.rs           # Public exports
├── error.rs         # UserError enum, Result type alias
├── models/
│   └── mod.rs       # User, UserSession, UserActivity, UserWithSessions, UserStatus, UserRole
├── repository/
│   ├── mod.rs       # UserRepository struct
│   └── user/
│       ├── mod.rs       # Module exports
│       ├── find.rs      # find_by_id, find_by_email, find_by_name, find_by_role
│       ├── list.rs      # list, list_all, search, count, get_with_sessions, get_activity
│       ├── operations.rs # create, update_*, delete, cleanup_old_anonymous
│       └── session.rs    # list_sessions, list_active_sessions, list_recent_sessions
├── services/
│   ├── mod.rs           # Service exports
│   ├── user_provider.rs # UserProviderImpl wrapper
│   └── user/
│       ├── mod.rs       # UserService
│       └── provider.rs  # UserProvider, RoleProvider trait implementations
└── jobs/
    ├── mod.rs                      # Job exports
    └── cleanup_anonymous_users.rs  # CleanupAnonymousUsersJob

schema/
├── users.sql                  # Users table
├── user_sessions.sql          # User sessions table
├── banned_ips.sql             # IP banning
├── session_analytics_views.sql
├── referrer_analytics_views.sql
└── bot_analytics_views.sql
```

## Public Exports

- `UserService` - Primary service (implements UserProvider, RoleProvider)
- `UserProviderImpl` - Wrapper for trait-based access
- `UserRepository` - Database access layer
- `User`, `UserSession`, `UserActivity`, `UserWithSessions` - Domain models
- `UserStatus`, `UserRole` - Type-safe enums
- `UserError`, `Result` - Error handling
- `UpdateUserParams` - Multi-field update struct
- `UserProvider`, `RoleProvider` - Re-exported traits

## Dependencies

- `systemprompt-core-database` - DbPool
- `systemprompt-core-logging` - Logging
- `systemprompt-traits` - UserProvider, RoleProvider traits
- `systemprompt-identifiers` - UserId, SessionId
- `systemprompt-models` - Shared models
