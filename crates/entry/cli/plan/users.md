# Users CLI Domain Plan

## Overview

Add comprehensive user management CLI commands exposing the existing `UserService`, `UserAdminService`, and `BannedIpRepository` functionality.

## Proposed Structure

```
users
├── list [--limit N] [--offset N] [--role ROLE] [--status STATUS]
├── show <USER_ID|EMAIL|NAME> [--sessions] [--activity]
├── search <QUERY> [--limit N]
├── create --name NAME --email EMAIL [--full-name NAME] [--display-name NAME]
├── update <USER_ID> [--email EMAIL] [--full-name NAME] [--status STATUS]
├── delete <USER_ID> --yes [--hard]
├── count
│
├── role
│   ├── assign <USER_ID> --roles ROLE1,ROLE2
│   ├── promote <USER_ID|EMAIL|NAME>
│   └── demote <USER_ID|EMAIL|NAME>
│
├── session
│   ├── list <USER_ID> [--active] [--limit N]
│   └── cleanup [--days N] --yes
│
└── ban
    ├── list [--limit N] [--source SOURCE]
    ├── add <IP> --reason REASON [--duration DURATION] [--permanent]
    ├── remove <IP>
    ├── check <IP>
    └── cleanup --yes
```

## File Structure

```
crates/entry/cli/src/commands/users/
├── mod.rs              # UsersCommands enum + dispatch
├── types.rs            # Output types (UserListOutput, UserDetailOutput, etc.)
├── list.rs             # users list
├── show.rs             # users show
├── search.rs           # users search
├── create.rs           # users create
├── update.rs           # users update
├── delete.rs           # users delete
├── count.rs            # users count
├── role/
│   ├── mod.rs          # RoleCommands enum + dispatch
│   ├── assign.rs       # users role assign
│   ├── promote.rs      # users role promote
│   └── demote.rs       # users role demote
├── session/
│   ├── mod.rs          # SessionCommands enum + dispatch
│   ├── list.rs         # users session list
│   └── cleanup.rs      # users session cleanup
└── ban/
    ├── mod.rs          # BanCommands enum + dispatch
    ├── list.rs         # users ban list
    ├── add.rs          # users ban add
    ├── remove.rs       # users ban remove
    ├── check.rs        # users ban check
    └── cleanup.rs      # users ban cleanup
```

## Command Details

### `users list`

List users with pagination and filtering.

```bash
users list                              # List first 20 users
users list --limit 50 --offset 100      # Paginate
users list --role admin                 # Filter by role
users list --status active              # Filter by status
users list --json                       # JSON output
```

**Args:**
```rust
#[derive(Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, value_enum)]
    pub role: Option<UserRole>,

    #[arg(long, value_enum)]
    pub status: Option<UserStatus>,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct UserListOutput {
    pub users: Vec<UserSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct UserSummary {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub status: String,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
}
```

**Service Call:** `UserService::list()` or `UserService::find_by_role()`

### `users show`

Show detailed user information.

```bash
users show user_abc123                  # By ID
users show john@example.com             # By email
users show johndoe                      # By name
users show user_abc123 --sessions       # Include sessions
users show user_abc123 --activity       # Include activity stats
```

**Args:**
```rust
#[derive(Args)]
pub struct ShowArgs {
    #[arg(help = "User ID, email, or name")]
    pub identifier: String,

    #[arg(long, help = "Include user sessions")]
    pub sessions: bool,

    #[arg(long, help = "Include activity statistics")]
    pub activity: bool,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct UserDetailOutput {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub display_name: Option<String>,
    pub status: String,
    pub email_verified: bool,
    pub roles: Vec<String>,
    pub is_bot: bool,
    pub is_scanner: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sessions: Option<Vec<SessionSummary>>,
    pub activity: Option<UserActivityOutput>,
}
```

**Service Calls:**
- `UserAdminService::find_user()` - Smart identifier parsing
- `UserService::get_with_sessions()` - If --sessions
- `UserService::get_activity()` - If --activity

### `users search`

Full-text search on users.

```bash
users search "john"                     # Search name, email, full_name
users search "example.com" --limit 10   # Limit results
```

**Args:**
```rust
#[derive(Args)]
pub struct SearchArgs {
    #[arg(help = "Search query")]
    pub query: String,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}
```

**Service Call:** `UserService::search()`

### `users create`

Create a new user.

```bash
users create --name johndoe --email john@example.com
users create --name johndoe --email john@example.com --full-name "John Doe"
```

**Args:**
```rust
#[derive(Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub email: Option<String>,

    #[arg(long)]
    pub full_name: Option<String>,

    #[arg(long)]
    pub display_name: Option<String>,
}
```

**Service Call:** `UserService::create()`

### `users update`

Update user fields.

```bash
users update user_abc123 --email new@example.com
users update user_abc123 --status suspended
users update user_abc123 --full-name "John Smith"
```

**Args:**
```rust
#[derive(Args)]
pub struct UpdateArgs {
    #[arg(help = "User ID")]
    pub user_id: String,

    #[arg(long)]
    pub email: Option<String>,

    #[arg(long)]
    pub full_name: Option<String>,

    #[arg(long)]
    pub display_name: Option<String>,

    #[arg(long, value_enum)]
    pub status: Option<UserStatus>,

    #[arg(long)]
    pub email_verified: Option<bool>,
}
```

**Service Calls:**
- `UserService::update_email()`
- `UserService::update_full_name()`
- `UserService::update_status()`
- `UserService::update_all_fields()`

### `users delete`

Delete a user (soft delete by default).

```bash
users delete user_abc123 --yes          # Soft delete
users delete user_abc123 --yes --hard   # Hard delete (anonymous only)
```

**Args:**
```rust
#[derive(Args)]
pub struct DeleteArgs {
    #[arg(help = "User ID")]
    pub user_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,

    #[arg(long, help = "Hard delete (anonymous users only)")]
    pub hard: bool,
}
```

**Service Calls:**
- `UserService::delete()` - Soft delete
- `UserService::delete_anonymous()` - Hard delete

### `users count`

Get total user count.

```bash
users count
```

**Service Call:** `UserService::count()`

### `users role assign`

Assign roles to a user.

```bash
users role assign user_abc123 --roles admin,user
```

**Service Call:** `UserService::assign_roles()`

### `users role promote`

Promote user to admin.

```bash
users role promote john@example.com
users role promote johndoe
```

**Service Call:** `UserAdminService::promote_to_admin()`

### `users role demote`

Demote user from admin.

```bash
users role demote john@example.com
```

**Service Call:** `UserAdminService::demote_from_admin()`

### `users session list`

List user sessions.

```bash
users session list user_abc123          # All sessions
users session list user_abc123 --active # Only active
users session list user_abc123 --limit 5
```

**Service Calls:**
- `UserService::list_sessions()`
- `UserService::list_active_sessions()`
- `UserService::list_recent_sessions()`

### `users session cleanup`

Clean up old anonymous users.

```bash
users session cleanup --days 30 --yes   # Clean anonymous users older than 30 days
```

**Service Call:** `UserService::cleanup_old_anonymous()`

### `users ban list`

List active IP bans.

```bash
users ban list                          # List active bans
users ban list --limit 50
users ban list --source anomaly_detector
```

**Service Calls:**
- `BannedIpRepository::list_active_bans()`
- `BannedIpRepository::list_bans_by_source()`

### `users ban add`

Ban an IP address.

```bash
users ban add 192.168.1.100 --reason "Suspicious activity"
users ban add 192.168.1.100 --reason "Spam" --duration 7d
users ban add 192.168.1.100 --reason "Abuse" --permanent
```

**Args:**
```rust
#[derive(Args)]
pub struct BanAddArgs {
    #[arg(help = "IP address to ban")]
    pub ip: String,

    #[arg(long)]
    pub reason: Option<String>,

    #[arg(long, help = "Duration (e.g., 1h, 7d, 30d)")]
    pub duration: Option<String>,

    #[arg(long, help = "Permanent ban")]
    pub permanent: bool,
}
```

**Service Call:** `BannedIpRepository::ban_ip()`

### `users ban remove`

Unban an IP address.

```bash
users ban remove 192.168.1.100
```

**Service Call:** `BannedIpRepository::unban_ip()`

### `users ban check`

Check if an IP is banned.

```bash
users ban check 192.168.1.100
```

**Service Call:** `BannedIpRepository::is_banned()`

### `users ban cleanup`

Clean up expired bans.

```bash
users ban cleanup --yes
```

**Service Call:** `BannedIpRepository::cleanup_expired()`

## Dependencies

Add to `crates/entry/cli/Cargo.toml`:
```toml
systemprompt_core_users = { path = "../../domain/users" }
```

## Implementation Checklist

- [ ] Create `commands/users/mod.rs` with `UsersCommands` enum
- [ ] Create `commands/users/types.rs` with output types
- [ ] Implement `users list`
- [ ] Implement `users show`
- [ ] Implement `users search`
- [ ] Implement `users create`
- [ ] Implement `users update`
- [ ] Implement `users delete`
- [ ] Implement `users count`
- [ ] Create `commands/users/role/mod.rs`
- [ ] Implement `users role assign`
- [ ] Implement `users role promote`
- [ ] Implement `users role demote`
- [ ] Create `commands/users/session/mod.rs`
- [ ] Implement `users session list`
- [ ] Implement `users session cleanup`
- [ ] Create `commands/users/ban/mod.rs`
- [ ] Implement `users ban list`
- [ ] Implement `users ban add`
- [ ] Implement `users ban remove`
- [ ] Implement `users ban check`
- [ ] Implement `users ban cleanup`
- [ ] Add `Users` variant to main `Commands` enum in `lib.rs`
- [ ] Update CLI README with users commands

## Verification

```bash
# List users
systemprompt users list
systemprompt users list --role admin --json

# Show user details
systemprompt users show admin@example.com --sessions --activity

# Search users
systemprompt users search "john"

# Create user
systemprompt users create --name testuser --email test@example.com

# Update user
systemprompt users update user_123 --status suspended

# Promote/demote admin
systemprompt users role promote testuser
systemprompt users role demote testuser

# Session management
systemprompt users session list user_123 --active

# IP ban management
systemprompt users ban list
systemprompt users ban add 10.0.0.1 --reason "Testing" --duration 1h
systemprompt users ban check 10.0.0.1
systemprompt users ban remove 10.0.0.1
```
