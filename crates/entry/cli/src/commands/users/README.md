# Users CLI Commands

This document provides complete documentation for AI agents to use the users CLI commands. All commands support non-interactive mode for automation.

**Important:** All user identifier arguments (`<USER>`) accept:
- Username (e.g., `johndoe`)
- UUID (e.g., `a602013b-f059-47eb-9169-df6e8f1372d4`)
- Email (e.g., `john@example.com`)

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

| Command | Description | Requires `--yes` |
|---------|-------------|------------------|
| `users list` | List users with pagination | No |
| `users show <USER>` | Show detailed user information | No |
| `users search <QUERY>` | Search users by name/email | No |
| `users create` | Create a new user | No |
| `users update <USER>` | Update user fields | No |
| `users delete <USER>` | Delete a user | **Yes** |
| `users count` | Get total user count | No |
| `users export` | Export users to JSON | No |
| `users stats` | Show user statistics dashboard | No |
| `users merge` | Merge source user into target | **Yes** |
| `users bulk delete` | Bulk delete users by filter | **Yes** |
| `users bulk update` | Bulk update user status | **Yes** |
| `users role assign <USER>` | Assign roles to user | No |
| `users role promote <USER>` | Promote user to admin | No |
| `users role demote <USER>` | Demote user from admin | No |
| `users session list <USER>` | List user sessions | No |
| `users session end` | End user session(s) | **Yes** |
| `users session cleanup` | Clean up old anonymous users | **Yes** |
| `users ban list` | List active IP bans | No |
| `users ban add <IP>` | Ban IP address | No |
| `users ban remove <IP>` | Remove IP ban | **Yes** |
| `users ban check <IP>` | Check if IP is banned | No |
| `users ban cleanup` | Clean up expired bans | **Yes** |

---

## Core Commands

### users list

List all users with pagination and filtering.

```bash
sp users list
sp --json users list
sp users list --limit 50 --offset 0
sp users list --role admin
sp users list --status active
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
      "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
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

---

### users show

Display detailed information for a specific user.

```bash
sp users show johndoe
sp users show john@example.com
sp users show a602013b-f059-47eb-9169-df6e8f1372d4
sp --json users show johndoe
sp users show johndoe --sessions
sp users show johndoe --activity
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<USER>` | Yes | Username, email, or UUID |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--sessions` | `false` | Include user sessions |
| `--activity` | `false` | Include activity statistics |

**Output Structure:**
```json
{
  "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
  "name": "johndoe",
  "email": "john@example.com",
  "full_name": "John Doe",
  "display_name": "John",
  "status": "active",
  "email_verified": false,
  "roles": ["user"],
  "is_bot": false,
  "is_scanner": false,
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-15T10:30:00Z",
  "sessions": [...],
  "activity": {...}
}
```

---

### users search

Search users by name, email, or full name.

```bash
sp users search "john"
sp --json users search "john"
sp users search "example.com" --limit 10
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<QUERY>` | Yes | Search query (substring match) |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "users": [
    {
      "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
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

---

### users create

Create a new user.

```bash
sp users create --name "johndoe" --email "john@example.com"
sp users create --name "johndoe" --email "john@example.com" --full-name "John Doe" --display-name "John"
```

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Username (unique) |
| `--email` | Yes | Email address (unique) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--full-name` | None | Full name |
| `--display-name` | None | Display name |

**Output Structure:**
```json
{
  "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
  "name": "johndoe",
  "email": "john@example.com",
  "message": "User 'johndoe' created successfully"
}
```

---

### users update

Update user fields. Accepts username, email, or UUID.

```bash
sp users update johndoe --email "newemail@example.com"
sp users update johndoe --full-name "John Smith" --display-name "Johnny"
sp users update johndoe --status suspended
sp users update a602013b-f059-47eb-9169-df6e8f1372d4 --email-verified true
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<USER>` | Yes | Username, email, or UUID |

**Modification Flags (at least one required):**
| Flag | Description |
|------|-------------|
| `--email` | Update email address |
| `--full-name` | Update full name |
| `--display-name` | Update display name |
| `--status` | Update status: `active`, `inactive`, `suspended` |
| `--email-verified` | Set email verification status |

**Output Structure:**
```json
{
  "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
  "name": "johndoe",
  "email": "newemail@example.com",
  "message": "User 'johndoe' updated successfully"
}
```

---

### users delete

Delete a user permanently. Requires `--yes` flag.

```bash
sp users delete johndoe --yes
sp users delete john@example.com --yes
sp users delete a602013b-f059-47eb-9169-df6e8f1372d4 --yes
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<USER>` | Yes | Username, email, or UUID |

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm deletion |

**Output Structure:**
```json
{
  "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
  "message": "User 'johndoe' deleted successfully"
}
```

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
  "count": 150
}
```

---

## Role Management Commands

### users role assign

Assign roles to a user. Accepts username, email, or UUID.

```bash
sp users role assign johndoe --roles admin,user
sp users role assign john@example.com --roles admin
sp users role assign a602013b-f059-47eb-9169-df6e8f1372d4 --roles user
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<USER>` | Yes | Username, email, or UUID |

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--roles` | Yes | Comma-separated roles: `admin`, `user`, `anonymous` |

**Output Structure:**
```json
{
  "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
  "name": "johndoe",
  "roles": ["admin", "user"],
  "message": "Roles assigned to user 'johndoe'"
}
```

---

### users role promote

Promote a user to admin. Accepts username, email, or UUID.

```bash
sp users role promote johndoe
sp users role promote john@example.com
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<USER>` | Yes | Username, email, or UUID |

**Output Structure:**
```json
{
  "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
  "name": "johndoe",
  "roles": ["user", "admin"],
  "message": "User 'johndoe' promoted to admin"
}
```

---

### users role demote

Demote a user from admin. Accepts username, email, or UUID.

```bash
sp users role demote johndoe
sp users role demote john@example.com
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<USER>` | Yes | Username, email, or UUID |

**Output Structure:**
```json
{
  "id": "a602013b-f059-47eb-9169-df6e8f1372d4",
  "name": "johndoe",
  "roles": ["user"],
  "message": "User 'johndoe' demoted from admin"
}
```

---

## Session Management Commands

### users session list

List sessions for a specific user. Accepts username, email, or UUID.

```bash
sp users session list johndoe
sp --json users session list johndoe
sp users session list johndoe --active
sp users session list johndoe --limit 10
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<USER>` | Yes | Username, email, or UUID |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--active` | `false` | Show only active sessions |
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "sessions": [
    {
      "session_id": "sess_4460f4d4-57ab-4996-a70a-5b6c086e4ae5",
      "ip_address": "192.168.1.1",
      "user_agent": "Mozilla/5.0...",
      "device_type": "desktop",
      "started_at": "2024-01-15T10:30:00Z",
      "last_activity_at": "2024-01-15T11:45:00Z",
      "is_active": true
    }
  ],
  "total": 1
}
```

---

### users session end

End a user session. Requires `--yes` flag.

```bash
# End specific session
sp users session end sess_4460f4d4-57ab-4996-a70a-5b6c086e4ae5 --yes

# End all sessions for a user
sp users session end --user johndoe --all --yes
sp users session end --user john@example.com --all --yes
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<SESSION_ID>` | Yes* | Session ID to end (*unless using `--all`) |

**Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm action |
| `--user` | No | User identifier (required with `--all`) |
| `--all` | No | End all sessions for user |

**Output Structure:**
```json
{
  "ended": ["sess_4460f4d4-57ab-4996-a70a-5b6c086e4ae5"],
  "count": 1,
  "message": "Session(s) ended successfully"
}
```

---

### users session cleanup

Clean up old anonymous users. Requires `--yes` flag.

```bash
sp users session cleanup --days 30 --yes
```

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--days` | Yes | Delete anonymous users older than N days |
| `--yes` / `-y` | Yes | Confirm action |

**Output Structure:**
```json
{
  "cleaned": 15,
  "message": "Cleaned up 15 old anonymous user(s)"
}
```

---

## IP Ban Management Commands

### users ban list

List active IP bans.

```bash
sp users ban list
sp --json users ban list
sp users ban list --limit 50
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `100` | Maximum number of results |

**Output Structure:**
```json
{
  "bans": [
    {
      "ip_address": "192.168.1.100",
      "reason": "Suspicious activity",
      "banned_at": "2024-01-15T10:30:00Z",
      "expires_at": "2024-01-22T10:30:00Z",
      "is_permanent": false,
      "ban_count": 1,
      "ban_source": "cli"
    }
  ],
  "total": 1
}
```

---

### users ban add

Ban an IP address.

```bash
sp users ban add 192.168.1.100 --reason "Suspicious activity"
sp users ban add 192.168.1.100 --reason "Abuse" --duration 7d
sp users ban add 192.168.1.100 --reason "Permanent ban" --permanent
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<IP>` | Yes | IP address to ban |

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--reason` | Yes | Reason for ban |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--duration` | `7d` | Ban duration (e.g., `1h`, `7d`, `30d`) |
| `--permanent` | `false` | Make ban permanent |

**Output Structure:**
```json
{
  "ip_address": "192.168.1.100",
  "reason": "Suspicious activity",
  "expires_at": "2024-01-22T10:30:00Z",
  "is_permanent": false,
  "message": "IP address '192.168.1.100' has been banned"
}
```

---

### users ban remove

Remove an IP ban. Requires `--yes` flag.

```bash
sp users ban remove 192.168.1.100 --yes
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<IP>` | Yes | IP address to unban |

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm action |

**Output Structure:**
```json
{
  "ip_address": "192.168.1.100",
  "removed": true,
  "message": "IP address '192.168.1.100' has been unbanned"
}
```

---

### users ban check

Check if an IP address is banned.

```bash
sp users ban check 192.168.1.100
sp --json users ban check 192.168.1.100
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<IP>` | Yes | IP address to check |

**Output Structure:**
```json
{
  "ip_address": "192.168.1.100",
  "is_banned": true,
  "ban_info": {
    "ip_address": "192.168.1.100",
    "reason": "Suspicious activity",
    "banned_at": "2024-01-15T10:30:00Z",
    "expires_at": "2024-01-22T10:30:00Z",
    "is_permanent": false,
    "ban_count": 1,
    "ban_source": "cli"
  }
}
```

---

### users ban cleanup

Clean up expired bans. Requires `--yes` flag.

```bash
sp users ban cleanup --yes
```

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm action |

**Output Structure:**
```json
{
  "cleaned": 5,
  "message": "Cleaned up 5 expired ban(s)"
}
```

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
sp --json users session list newuser

# Phase 9: Delete user
sp users delete newuser --yes

# Phase 10: Verify deletion
sp --json users list
```

---

## Session and Security Flow Example

```bash
# Check for active sessions
sp --json users session list johndoe --active

# End specific session
sp users session end sess_suspicious123 --yes

# End all sessions for a user
sp users session end --user johndoe --all --yes

# Ban suspicious IP
sp users ban add 10.0.0.100 --reason "Multiple failed login attempts"

# Check ban status
sp --json users ban check 10.0.0.100

# List all bans
sp --json users ban list

# Remove ban after investigation
sp users ban remove 10.0.0.100 --yes

# Cleanup expired bans
sp users ban cleanup --yes
```

---

## Error Handling

### Missing Required Flags

```bash
sp users create --name test
# Error: --email is required

sp users delete johndoe
# Error: --yes is required to delete users in non-interactive mode

sp users role assign johndoe
# Error: At least one role must be specified

sp users ban add 10.0.0.1
# Error: --reason is required
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
sp --json users show johndoe | jq '.roles'
sp --json users count | jq '.count'

# Filter by criteria
sp --json users list | jq '.users[] | select(.status == "active")'
sp --json users list | jq '.users[] | select(.roles | contains(["admin"]))'
sp --json users session list johndoe | jq '.sessions[] | select(.is_active == true)'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All destructive commands require `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with proper error handling
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
- [x] All user identifiers accept username, email, or UUID
