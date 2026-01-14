# Users CLI Commands

This document provides complete documentation for AI agents to use the users CLI commands. All commands support non-interactive mode for automation.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `users list` | List users with pagination | `Table` | No (DB only) |
| `users show <id>` | Show detailed user information | `Card` | No (DB only) |
| `users search <query>` | Search users by name/email | `Table` | No (DB only) |
| `users create` | Create a new user | `Text` | No (DB only) |
| `users update <id>` | Update user fields | `Text` | No (DB only) |
| `users delete <id>` | Delete a user (soft delete) | `Text` | No (DB only) |
| `users count` | Get total user count | `Card` | No (DB only) |
| `users role assign` | Assign role to user | `Text` | No (DB only) |
| `users role promote` | Promote user to admin | `Text` | No (DB only) |
| `users role demote` | Demote user from admin | `Text` | No (DB only) |
| `users session list` | List user sessions | `Table` | No (DB only) |
| `users session end` | End user session | `Text` | No (DB only) |
| `users ban add` | Ban IP address | `Text` | No (DB only) |
| `users ban remove` | Remove IP ban | `Text` | No (DB only) |
| `users ban check` | Check if IP is banned | `Card` | No (DB only) |

---

## Core Commands

### users list

List all users with pagination and filtering.

```bash
sp users list
sp --json users list
sp users list --limit 50 --offset 0
sp users list --role admin
sp users list --role user
sp users list --status active
sp users list --status suspended
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--offset` | `0` | Number of results to skip |
| `--role` | None | Filter by role: `admin`, `user`, `anonymous` |
| `--status` | None | Filter by status: `active`, `inactive`, `suspended`, `pending`, `deleted`, `temporary` |

**Output Structure:**
```json
{
  "users": [
    {
      "id": "user_abc123",
      "name": "johndoe",
      "email": "john@example.com",
      "status": "active",
      "roles": ["user"],
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `name`, `email`, `status`, `roles`

---

### users show

Display detailed information for a specific user.

```bash
sp users show <user-id>
sp --json users show user_abc123
sp users show johndoe
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | User ID or username |

**Output Structure:**
```json
{
  "id": "user_abc123",
  "name": "johndoe",
  "email": "john@example.com",
  "full_name": "John Doe",
  "display_name": "John",
  "status": "active",
  "roles": ["user"],
  "sessions_count": 5,
  "last_login": "2024-01-15T10:30:00Z",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

### users search

Search users by name, email, or full name.

```bash
sp users search <query>
sp --json users search "john"
sp users search "example.com"
sp users search "doe" --limit 10
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<query>` | Yes | Search query (substring match) |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "users": [
    {
      "id": "user_abc123",
      "name": "johndoe",
      "email": "john@example.com",
      "full_name": "John Doe",
      "match_field": "name"
    }
  ],
  "query": "john",
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `name`, `email`, `full_name`, `match_field`

---

### users create

Create a new user.

```bash
sp users create --name <name> --email <email>
sp users create --name "johndoe" --email "john@example.com"
sp users create --name "johndoe" --email "john@example.com" --full-name "John Doe" --display-name "John"
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Username (unique) |
| `--email` | Yes | Email address (unique) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--full-name` | None | Full name |
| `--display-name` | None | Display name |

**Validation Rules:**
- Name: Non-empty, unique
- Email: Non-empty, valid format, unique

**Output Structure:**
```json
{
  "id": "user_abc123",
  "name": "johndoe",
  "email": "john@example.com",
  "message": "User 'johndoe' created successfully"
}
```

**Artifact Type:** `Text`

---

### users update

Update user fields.

```bash
sp users update <user-id> --email <new-email>
sp users update user_abc123 --email "newemail@example.com"
sp users update user_abc123 --full-name "John Smith" --display-name "Johnny"
sp users update user_abc123 --status suspended
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | User ID to update |
| At least one change | Yes | Must specify at least one modification |

**Modification Flags:**
| Flag | Description |
|------|-------------|
| `--email` | Update email address |
| `--full-name` | Update full name |
| `--display-name` | Update display name |
| `--status` | Update status: `active`, `inactive`, `suspended` |

**Output Structure:**
```json
{
  "id": "user_abc123",
  "message": "User 'johndoe' updated successfully",
  "changes": ["email: newemail@example.com", "status: suspended"]
}
```

**Artifact Type:** `Text`

---

### users delete

Delete a user (soft delete by default).

```bash
sp users delete <user-id> --yes
sp users delete user_abc123 --yes
sp users delete user_abc123 --yes --hard
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<id>` | Yes | User ID to delete |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--hard` | `false` | Permanently delete (cannot be recovered) |

**Output Structure:**
```json
{
  "deleted": "user_abc123",
  "hard_delete": false,
  "message": "User 'johndoe' deleted successfully"
}
```

**Artifact Type:** `Text`

---

### users count

Get total user count.

```bash
sp users count
sp --json users count
```

**Output Structure:**
```json
{
  "total": 150,
  "by_status": {
    "active": 120,
    "inactive": 25,
    "suspended": 5
  },
  "by_role": {
    "admin": 3,
    "user": 147
  }
}
```

**Artifact Type:** `Card`

---

## Role Management Commands

### users role assign

Assign a role to a user.

```bash
sp users role assign --user <user-id> --role <role>
sp users role assign --user user_abc123 --role admin
sp users role assign --user user_abc123 --role user
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--user` | Yes | User ID |
| `--role` | Yes | Role to assign: `admin`, `user` |

**Output Structure:**
```json
{
  "user_id": "user_abc123",
  "role": "admin",
  "message": "Role 'admin' assigned to user successfully"
}
```

**Artifact Type:** `Text`

---

### users role promote

Promote a user to admin.

```bash
sp users role promote <user-id>
sp users role promote user_abc123
sp users role promote johndoe
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<user>` | Yes | User ID or username |

**Output Structure:**
```json
{
  "user_id": "user_abc123",
  "name": "johndoe",
  "previous_roles": ["user"],
  "new_roles": ["user", "admin"],
  "message": "User 'johndoe' promoted to admin"
}
```

**Artifact Type:** `Text`

---

### users role demote

Demote a user from admin.

```bash
sp users role demote <user-id>
sp users role demote user_abc123
sp users role demote johndoe
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<user>` | Yes | User ID or username |

**Output Structure:**
```json
{
  "user_id": "user_abc123",
  "name": "johndoe",
  "previous_roles": ["user", "admin"],
  "new_roles": ["user"],
  "message": "Admin role removed from user 'johndoe'"
}
```

**Artifact Type:** `Text`

---

## Session Management Commands

### users session list

List user sessions.

```bash
sp users session list
sp --json users session list
sp users session list --user user_abc123
sp users session list --active
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--user` | None | Filter by user ID |
| `--active` | `false` | Show only active sessions |
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "sessions": [
    {
      "session_id": "sess_abc123",
      "user_id": "user_xyz789",
      "started_at": "2024-01-15T10:30:00Z",
      "last_activity": "2024-01-15T11:45:00Z",
      "ip_address": "192.168.1.1",
      "user_agent": "Mozilla/5.0...",
      "active": true
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `session_id`, `user_id`, `started_at`, `last_activity`, `active`

---

### users session end

End a user session.

```bash
sp users session end <session-id> --yes
sp users session end sess_abc123 --yes
sp users session end --user user_abc123 --all --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<session-id>` | Yes* | Session ID (*unless using --all) |
| `--yes` / `-y` | Yes | Skip confirmation |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--user` | None | End sessions for specific user |
| `--all` | `false` | End all sessions for user (requires --user) |

**Output Structure:**
```json
{
  "ended": ["sess_abc123"],
  "count": 1,
  "message": "Session(s) ended successfully"
}
```

**Artifact Type:** `Text`

---

## IP Ban Management Commands

### users ban add

Ban an IP address.

```bash
sp users ban add <ip-address>
sp users ban add 192.168.1.100
sp users ban add 192.168.1.100 --reason "Suspicious activity"
sp users ban add 192.168.1.100 --expires "2024-02-15T00:00:00Z"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<ip>` | Yes | IP address to ban |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--reason` | None | Reason for ban |
| `--expires` | None | Ban expiration (ISO datetime) |

**Output Structure:**
```json
{
  "ip_address": "192.168.1.100",
  "reason": "Suspicious activity",
  "expires_at": "2024-02-15T00:00:00Z",
  "message": "IP address '192.168.1.100' banned successfully"
}
```

**Artifact Type:** `Text`

---

### users ban remove

Remove an IP ban.

```bash
sp users ban remove <ip-address> --yes
sp users ban remove 192.168.1.100 --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<ip>` | Yes | IP address to unban |
| `--yes` / `-y` | Yes | Skip confirmation |

**Output Structure:**
```json
{
  "ip_address": "192.168.1.100",
  "message": "IP ban removed successfully"
}
```

**Artifact Type:** `Text`

---

### users ban check

Check if an IP address is banned.

```bash
sp users ban check <ip-address>
sp --json users ban check 192.168.1.100
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<ip>` | Yes | IP address to check |

**Output Structure:**
```json
{
  "ip_address": "192.168.1.100",
  "is_banned": true,
  "reason": "Suspicious activity",
  "banned_at": "2024-01-15T10:30:00Z",
  "expires_at": "2024-02-15T00:00:00Z"
}
```

**Artifact Type:** `Card`

---

## Complete User Management Flow Example

This flow demonstrates the full user lifecycle:

```bash
# Phase 1: List existing users
sp --json users list
sp --json users count

# Phase 2: Create new user
sp users create --name "newuser" --email "new@example.com" --full-name "New User"

# Phase 3: Verify creation
sp --json users show newuser
sp --json users search "newuser"

# Phase 4: Update user
sp users update newuser --display-name "Newbie"

# Phase 5: Promote to admin
sp users role promote newuser

# Phase 6: Verify role
sp --json users show newuser
# Should show roles: ["user", "admin"]

# Phase 7: Demote from admin
sp users role demote newuser

# Phase 8: Check sessions
sp --json users session list --user newuser

# Phase 9: Delete user
sp users delete newuser --yes

# Phase 10: Verify deletion
sp --json users list
```

---

## Session and Security Flow Example

```bash
# Check for active sessions
sp --json users session list --active

# End suspicious session
sp users session end sess_suspicious123 --yes

# Ban suspicious IP
sp users ban add 10.0.0.100 --reason "Multiple failed login attempts"

# Check ban status
sp --json users ban check 10.0.0.100

# Remove ban after investigation
sp users ban remove 10.0.0.100 --yes
```

---

## Error Handling

### Missing Required Flags

```bash
sp users create --name test
# Error: --email is required

sp users delete user_abc123
# Error: --yes is required to delete users in non-interactive mode

sp users role assign --user user_abc
# Error: --role is required
```

### Validation Errors

```bash
sp users create --name "" --email "test@example.com"
# Error: Name cannot be empty

sp users create --name "test" --email "invalid-email"
# Error: Invalid email format
```

### Not Found Errors

```bash
sp users show nonexistent
# Error: User 'nonexistent' not found

sp users role promote nonexistent
# Error: User 'nonexistent' not found
```

### Duplicate Errors

```bash
sp users create --name "existing" --email "existing@example.com"
# Error: User with email 'existing@example.com' already exists
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json users list | jq .

# Extract specific fields
sp --json users list | jq '.users[].email'
sp --json users show user_abc | jq '.roles'
sp --json users count | jq '.by_role.admin'

# Filter by criteria
sp --json users list | jq '.users[] | select(.status == "active")'
sp --json users list | jq '.users[] | select(.roles | contains(["admin"]))'
sp --json users session list | jq '.sessions[] | select(.active == true)'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
