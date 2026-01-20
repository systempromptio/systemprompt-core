# System CLI Commands

This document provides complete documentation for AI agents to use the system CLI commands. All commands support non-interactive mode for automation.

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
| `infra system login` | Create session and get auth token | `Card` | No (DB only) |

---

## Core Commands

### system login

Create a session and get an authentication token for API access.

```bash
sp infra system login --user <user-id>
sp --json system login --user user_abc123
sp infra system login --user johndoe --device "CLI Client"
sp infra system login --email john@example.com
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--user` or `--email` | Yes | User ID or email to create session for |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--device` | `CLI` | Device/client identifier |
| `--duration` | `24h` | Session duration |

**Output Structure:**
```json
{
  "session_id": "sess_abc123xyz",
  "user_id": "user_abc123",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_at": "2024-01-16T10:30:00Z",
  "message": "Session created successfully"
}
```

**Artifact Type:** `Card`

---

## Using the Authentication Token

### With curl

```bash
# Get token
TOKEN=$(sp --json system login --user johndoe | jq -r '.token')

# Use token in API requests
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v1/users/me
```

### With the Agents API

```bash
# Login and store token
sp --json system login --user johndoe > /tmp/session.json
TOKEN=$(jq -r '.token' /tmp/session.json)

# Send message to agent
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello"}' \
  http://localhost:8080/api/v1/agents/primary/message
```

---

## Complete Authentication Flow Example

```bash
# Phase 1: Create or find user
sp --json users list | jq '.users[0]'
# or create a new user
sp admin users create --name "apiuser" --email "api@example.com"

# Phase 2: Create session
sp --json system login --user apiuser --device "Automation Script"

# Phase 3: Extract and use token
TOKEN=$(sp --json system login --user apiuser | jq -r '.token')
echo "Token: $TOKEN"

# Phase 4: Verify token works
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v1/health

# Phase 5: Use with agent
curl -s -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "method": "message/send", "params": {"message": {"role": "user", "parts": [{"kind": "text", "text": "Hello"}]}}, "id": "1"}' \
  http://localhost:8080/api/v1/agents/primary
```

---

## Token Format

The authentication token is a JWT (JSON Web Token) containing:

```json
{
  "sub": "user_abc123",
  "session_id": "sess_xyz789",
  "iat": 1705312200,
  "exp": 1705398600,
  "type": "session"
}
```

---

## Session Duration Options

| Duration | Description |
|----------|-------------|
| `1h` | 1 hour (short-lived) |
| `24h` | 24 hours (default) |
| `7d` | 7 days |
| `30d` | 30 days |

```bash
# Short session for testing
sp infra system login --user johndoe --duration 1h

# Long session for automation
sp infra system login --user apiuser --duration 30d
```

---

## Error Handling

### User Not Found

```bash
sp infra system login --user nonexistent
# Error: User 'nonexistent' not found
```

### Missing Required Flags

```bash
sp infra system login
# Error: --user or --email is required in non-interactive mode
```

### Database Connection Error

```bash
sp infra system login --user johndoe
# Error: Failed to connect to database. Check your profile configuration.
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json system login --user johndoe | jq .

# Extract specific fields
sp --json system login --user johndoe | jq '.token'
sp --json system login --user johndoe | jq '.session_id'
sp --json system login --user johndoe | jq '.expires_at'
```

---

## Security Considerations

1. **Token Storage**: Store tokens securely, not in plaintext files
2. **Short Duration**: Use shorter durations for sensitive operations
3. **Device Tracking**: Use meaningful device names for audit trails
4. **Token Rotation**: Regularly create new sessions for long-running automation

```bash
# Good: Descriptive device name
sp infra system login --user apiuser --device "CI/CD Pipeline - Build Server 1"

# Good: Short duration for sensitive operation
sp infra system login --user admin --duration 1h --device "Admin Task"
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
