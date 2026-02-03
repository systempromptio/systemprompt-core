<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


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
| `infra db query <sql>` | Execute SQL query (read-only) | `Table` | No (DB only) |
| `infra db execute <sql>` | Execute write operation | `Table` | No (DB only) |
| `infra db tables` | List all tables with sizes | `Table` | No (DB only) |
| `infra db describe <table>` | Describe table schema with indexes | `Table` | No (DB only) |
| `infra db info` | Database information | `Card` | No (DB only) |
| `infra db migrate` | Run database migrations | `Text` | No (DB only) |
| `infra db assign-admin <user>` | Assign admin role to user | `Text` | No (DB only) |
| `infra db status` | Show database connection status | `Card` | No (DB only) |
| `infra db validate` | Validate schema against expected tables | `Text` | No (DB only) |

---

## Core Commands

### db query

Execute a read-only SQL query.

```bash
sp infra db query "SELECT * FROM users LIMIT 10"
sp --json db query "SELECT * FROM users LIMIT 10"
sp infra db query "SELECT COUNT(*) FROM user_sessions" --format json
sp infra db query "SELECT id, name FROM users WHERE status = 'active'"
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
sp infra db execute "UPDATE users SET status = 'active' WHERE id = 'user_abc'"
sp infra db execute "DELETE FROM user_sessions WHERE ended_at < NOW() - INTERVAL '7 days'"
sp --json db execute "INSERT INTO settings (key, value) VALUES ('feature_x', 'enabled')"
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
  "message": "Query executed successfully, 5 row(s) affected"
}
```

**Artifact Type:** `Table`

---

### db tables

List all tables in the database with row counts and sizes.

```bash
sp infra db tables
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

---

### db describe

Describe table schema with columns and indexes.

```bash
sp infra db describe <table-name>
sp --json db describe users
sp infra db describe user_sessions
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
      "type": "text",
      "nullable": false,
      "default": null,
      "primary_key": true
    },
    {
      "name": "name",
      "type": "character varying",
      "nullable": false,
      "default": null,
      "primary_key": false
    },
    {
      "name": "email",
      "type": "character varying",
      "nullable": false,
      "default": null,
      "primary_key": false
    }
  ],
  "indexes": [
    {"name": "users_pkey", "columns": ["id"], "unique": true},
    {"name": "users_email_key", "columns": ["email"], "unique": true}
  ]
}
```

**Artifact Type:** `Table`

---

### db info

Show database information.

```bash
sp infra db info
sp --json db info
```

**Output Structure:**
```json
{
  "version": "PostgreSQL 17.7 on x86_64-pc-linux-musl...",
  "database": "PostgreSQL",
  "size": "45.41 MB",
  "table_count": 85,
  "tables": ["users", "user_sessions", "..."]
}
```

**Artifact Type:** `Card`

---

### db migrate

Run database migrations.

```bash
sp infra db migrate
sp --json db migrate
```

**Migration Process:**
1. Loads all registered modules
2. Executes schema migrations in order
3. Creates/updates tables as needed
4. Reports results

**Output Structure:**
```json
{
  "modules_installed": ["database", "users", "mcp", "ai", "..."],
  "message": "Database migration completed successfully"
}
```

**Artifact Type:** `Text`

---

### db assign-admin

Assign admin role to a user.

```bash
sp infra db assign-admin <user>
sp --json db assign-admin johndoe
sp --json db assign-admin john@example.com
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<user>` | Yes | Username or email |

**Output Structure:**
```json
{
  "user_id": "5ee65aa3-4f0a-47af-ab90-ac91d21fc227",
  "name": "johndoe",
  "email": "john@example.com",
  "roles": ["user", "admin"],
  "already_admin": false,
  "message": "Admin role assigned to user 'johndoe' (john@example.com)"
}
```

**Artifact Type:** `Text`

---

### db status

Show database connection status.

```bash
sp infra db status
sp --json db status
```

**Output Structure:**
```json
{
  "status": "connected",
  "version": "PostgreSQL 17.7 on x86_64-pc-linux-musl...",
  "tables": 85,
  "size": "45.41 MB"
}
```

**Artifact Type:** `Card`

---

### db validate

Validate database schema against expected tables.

```bash
sp infra db validate
sp --json db validate
```

**Output Structure:**
```json
{
  "valid": true,
  "expected_tables": 25,
  "actual_tables": 85,
  "missing_tables": [],
  "extra_tables": ["anomaly_thresholds", "banned_ips", "..."],
  "message": "Database schema is valid"
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
sp infra db migrate

# Phase 7: Validate schema
sp --json db validate

# Phase 8: Assign admin role
sp --json db assign-admin developer@example.com
```

---

## Query Examples

### Common Read Queries

```bash
# Count users by status
sp infra db query "SELECT status, COUNT(*) FROM users GROUP BY status"

# Get recent sessions
sp infra db query "SELECT * FROM user_sessions ORDER BY started_at DESC LIMIT 10"

# Find content by source
sp infra db query "SELECT id, slug, title FROM markdown_content WHERE source_id = 'blog'"

# Check AI request costs
sp infra db query "SELECT DATE(created_at), SUM(cost_microdollars) FROM ai_requests GROUP BY DATE(created_at) ORDER BY 1 DESC LIMIT 7"
```

### Common Write Operations

```bash
# Update user status
sp infra db execute "UPDATE users SET status = 'suspended' WHERE id = 'user_abc'"

# Clean old sessions
sp infra db execute "DELETE FROM user_sessions WHERE ended_at < NOW() - INTERVAL '30 days'"

# Update setting
sp infra db execute "UPDATE settings SET value = 'enabled' WHERE key = 'feature_x'"
```

---

## Error Handling

### Connection Errors

```bash
sp infra db status
# Error: Failed to connect to database. Check your profile configuration.
```

### Query Errors

```bash
sp infra db query "SELECT * FROM nonexistent_table"
# Error: Table or column not found: nonexistent_table

sp infra db query "INVALID SQL"
# Error: Query failed: Write query not allowed in read-only mode
```

### Table Not Found

```bash
sp infra db describe nonexistent
# Error: Table 'nonexistent' not found
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
sp --json db info | jq '.table_count'

# Query and process results
sp --json db query "SELECT * FROM users LIMIT 5" | jq '.rows[].email'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All output types derive `Serialize`, `Deserialize`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] No destructive operations (reset removed for safety)
- [x] User-friendly error messages
- [x] Schema validation via `infra db validate`
- [x] Table sizes and index information included
