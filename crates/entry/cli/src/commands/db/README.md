# Database CLI Commands

This document provides complete documentation for AI agents to use the database CLI commands. All commands support non-interactive mode for automation.

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
| `db query <sql>` | Execute SQL query (read-only) | `Table` | No (DB only) |
| `db execute <sql>` | Execute write operation | `Table` | No (DB only) |
| `db tables` | List all tables | `Table` | No (DB only) |
| `db describe <table>` | Describe table schema | `Table` | No (DB only) |
| `db info` | Database information | `Card` | No (DB only) |
| `db migrate` | Run database migrations | `Text` | No (DB only) |
| `db assign-admin <user>` | Assign admin role to user | `Text` | No (DB only) |
| `db status` | Show database connection status | `Card` | No (DB only) |
| `db reset` | Reset database (drop and recreate) | `Text` | No (DB only) |

---

## Core Commands

### db query

Execute a read-only SQL query.

```bash
sp db query "SELECT * FROM users LIMIT 10"
sp --json db query "SELECT * FROM users LIMIT 10"
sp db query "SELECT COUNT(*) FROM user_sessions" --format json
sp db query "SELECT id, name FROM users WHERE status = 'active'"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<sql>` | Yes | SQL query to execute |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--format` | `table` | Output format: `table`, `json`, `yaml` |

**Output Structure:**
```json
{
  "columns": ["id", "name", "email", "created_at"],
  "rows": [
    {
      "id": "user_abc123",
      "name": "johndoe",
      "email": "john@example.com",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "row_count": 1,
  "execution_time_ms": 15
}
```

**Artifact Type:** `Table`

---

### db execute

Execute a write operation (INSERT, UPDATE, DELETE).

```bash
sp db execute "UPDATE users SET status = 'active' WHERE id = 'user_abc'"
sp db execute "DELETE FROM user_sessions WHERE ended_at < NOW() - INTERVAL '7 days'"
sp db execute "INSERT INTO settings (key, value) VALUES ('feature_x', 'enabled')"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<sql>` | Yes | SQL statement to execute |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--format` | `table` | Output format: `table`, `json`, `yaml` |

**Output Structure:**
```json
{
  "rows_affected": 5,
  "execution_time_ms": 25,
  "message": "Query executed successfully"
}
```

**Artifact Type:** `Table`

---

### db tables

List all tables in the database.

```bash
sp db tables
sp --json db tables
```

**Output Structure:**
```json
{
  "tables": [
    {
      "name": "users",
      "schema": "public",
      "row_count": 150,
      "size_bytes": 524288
    },
    {
      "name": "user_sessions",
      "schema": "public",
      "row_count": 1200,
      "size_bytes": 1048576
    }
  ],
  "total": 25
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `schema`, `row_count`, `size_bytes`

---

### db describe

Describe table schema.

```bash
sp db describe <table-name>
sp --json db describe users
sp db describe user_sessions
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<table>` | Yes | Table name to describe |

**Output Structure:**
```json
{
  "table": "users",
  "row_count": 150,
  "columns": [
    {
      "name": "id",
      "type": "uuid",
      "nullable": false,
      "default": "gen_random_uuid()",
      "primary_key": true
    },
    {
      "name": "name",
      "type": "varchar(255)",
      "nullable": false,
      "default": null,
      "primary_key": false
    },
    {
      "name": "email",
      "type": "varchar(255)",
      "nullable": false,
      "default": null,
      "primary_key": false
    },
    {
      "name": "created_at",
      "type": "timestamptz",
      "nullable": false,
      "default": "now()",
      "primary_key": false
    }
  ],
  "indexes": [
    {"name": "users_pkey", "columns": ["id"], "unique": true},
    {"name": "users_email_idx", "columns": ["email"], "unique": true}
  ]
}
```

**Artifact Type:** `Table`

---

### db info

Show database information.

```bash
sp db info
sp --json db info
```

**Output Structure:**
```json
{
  "version": "PostgreSQL 15.4",
  "host": "localhost",
  "port": 5432,
  "database": "systemprompt_dev",
  "user": "systemprompt_dev",
  "size": "125 MB",
  "tables": 25,
  "uptime": "15 days 4 hours",
  "connections": {
    "active": 5,
    "idle": 10,
    "max": 100
  }
}
```

**Artifact Type:** `Card`

---

### db migrate

Run database migrations.

```bash
sp db migrate
```

**Migration Process:**
1. Loads all registered modules
2. Executes schema migrations in order
3. Creates/updates tables as needed
4. Reports results

**Output Structure:**
```json
{
  "migrations_run": 15,
  "tables_created": 3,
  "tables_updated": 5,
  "errors": [],
  "message": "Database migration completed successfully"
}
```

**Artifact Type:** `Text`

---

### db assign-admin

Assign admin role to a user.

```bash
sp db assign-admin <user>
sp db assign-admin johndoe
sp db assign-admin john@example.com
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<user>` | Yes | Username or email |

**Output Structure:**
```json
{
  "user_id": "user_abc123",
  "name": "johndoe",
  "email": "john@example.com",
  "roles": ["user", "admin"],
  "message": "Admin role assigned to user 'johndoe'"
}
```

**Artifact Type:** `Text`

---

### db status

Show database connection status.

```bash
sp db status
sp --json db status
```

**Output Structure:**
```json
{
  "status": "connected",
  "version": "PostgreSQL 15.4",
  "tables": 25,
  "size": "125 MB",
  "latency_ms": 5
}
```

**Artifact Type:** `Card`

---

### db reset

Reset database (drop all tables and recreate).

```bash
sp db reset --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` | Yes | Confirm reset (REQUIRED in non-interactive mode) |

**Output Structure:**
```json
{
  "reset": true,
  "tables_dropped": 25,
  "migrations_run": 15,
  "message": "Database reset completed successfully"
}
```

**Artifact Type:** `Text`

---

## Complete Database Management Flow Example

```bash
# Phase 1: Check connection status
sp --json db status

# Phase 2: View database info
sp --json db info

# Phase 3: List all tables
sp --json db tables

# Phase 4: Describe specific table
sp --json db describe users

# Phase 5: Run a query
sp --json db query "SELECT COUNT(*) as count FROM users"

# Phase 6: Run migrations
sp db migrate

# Phase 7: Assign admin role
sp db assign-admin developer@example.com

# Phase 8: Verify admin assignment
sp --json db query "SELECT * FROM users WHERE email = 'developer@example.com'"
```

---

## Query Examples

### Common Read Queries

```bash
# Count users by status
sp db query "SELECT status, COUNT(*) FROM users GROUP BY status"

# Get recent sessions
sp db query "SELECT * FROM user_sessions ORDER BY started_at DESC LIMIT 10"

# Find content by source
sp db query "SELECT id, slug, title FROM markdown_content WHERE source_id = 'blog'"

# Check AI request costs
sp db query "SELECT DATE(created_at), SUM(cost_cents) FROM ai_requests GROUP BY DATE(created_at) ORDER BY 1 DESC LIMIT 7"
```

### Common Write Operations

```bash
# Update user status
sp db execute "UPDATE users SET status = 'suspended' WHERE id = 'user_abc'"

# Clean old sessions
sp db execute "DELETE FROM user_sessions WHERE ended_at < NOW() - INTERVAL '30 days'"

# Update setting
sp db execute "UPDATE settings SET value = 'enabled' WHERE key = 'feature_x'"
```

---

## Error Handling

### Connection Errors

```bash
sp db status
# Error: Failed to connect to database. Check your profile configuration.
```

### Query Errors

```bash
sp db query "SELECT * FROM nonexistent_table"
# Error: relation "nonexistent_table" does not exist

sp db query "INVALID SQL"
# Error: syntax error at or near "INVALID"
```

### Table Not Found

```bash
sp db describe nonexistent
# Error: Table 'nonexistent' not found
```

### Reset Without Confirmation

```bash
sp db reset
# Error: --yes is required to reset database in non-interactive mode
# Warning: This will drop ALL tables and recreate the schema!
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json db tables | jq .

# Extract specific fields
sp --json db tables | jq '.tables[].name'
sp --json db describe users | jq '.columns[].name'
sp --json db info | jq '.connections'

# Query and process results
sp --json db query "SELECT * FROM users LIMIT 5" | jq '.rows[].email'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `reset` command requires `--yes` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
